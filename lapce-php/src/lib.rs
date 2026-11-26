//! Cached PHP structural context. Painting remains in gravity-lex/Lexilla.
/// One ABI bridge, shared with lapce-index. No second native runtime.
pub fn language(php_only: bool) -> tree_sitter::Language {
    let grammar = if php_only {
        tree_sitter_php::LANGUAGE_PHP_ONLY
    } else {
        tree_sitter_php::LANGUAGE_PHP
    };
    // SAFETY: both pinned generated grammars return immutable static ABI-14
    // TSLanguage data, accepted by the single tree-sitter 0.22.6 runtime.
    unsafe {
        let pointer = (grammar.into_raw())();
        assert!(!pointer.is_null());
        tree_sitter::Language::from_raw(pointer.cast())
    }
}

use lapce_xi_rope::Rope;
use std::{
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};
use tree_sitter::{InputEdit, Point, Tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    Php,
    Html,
    Sql,
    Javascript,
    Css,
    PhpDoc,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageRegion {
    pub range: Range<usize>,
    pub context: Context,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegionIndex {
    regions: Vec<LanguageRegion>,
    len: usize,
}
impl RegionIndex {
    pub fn regions(&self) -> &[LanguageRegion] {
        &self.regions
    }
    /// EOF belongs to the preceding region, useful for incomplete cursor input.
    pub fn at(&self, offset: usize) -> Option<Context> {
        if self.len == 0 || offset > self.len {
            return None;
        }
        let offset = offset.min(self.len - 1);
        let index = self
            .regions
            .partition_point(|region| region.range.start <= offset)
            .checked_sub(1)?;
        let region = &self.regions[index];
        region.range.contains(&offset).then_some(region.context)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CursorContext {
    NonPhp(Context),
    Variable,
    InstanceMember,
    StaticMember,
    New,
    Import,
    Extends,
    Implements,
    Type,
    Expression,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Host {
    Text,
    Tag { buffer: String, quote: Option<u8> },
    Comment,
    Raw(Context),
}
impl Default for Host {
    fn default() -> Self {
        Self::Text
    }
}
#[derive(Clone, Debug)]
struct Unit {
    host_before: Host,
    host_after: Host,
    range: Range<usize>,
    regions: Vec<LanguageRegion>,
}
/// Immutable publication value. Clone is cheap for the Tree/Rope; call update
/// only on a background worker. A stale/cancelled result is never published.
#[derive(Clone)]
pub struct Document {
    tree: Tree,
    text: Rope,
    index: RegionIndex,
    units: Vec<Unit>,
    php_only: bool,
    pub rescanned_units: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Cancelled,
    InvalidEdit,
    Parse(String),
}

pub fn byte_to_point(text: &Rope, offset: usize) -> Option<Point> {
    if offset > text.len() || !text.is_codepoint_boundary(offset) {
        return None;
    }
    let row = text.line_of_offset(offset);
    Some(Point::new(row, offset - text.offset_of_line(row)))
}
pub fn point_to_byte(text: &Rope, point: Point) -> Option<usize> {
    if point.row > text.line_of_offset(text.len()) {
        return None;
    }
    let at = text.offset_of_line(point.row).checked_add(point.column)?;
    (at <= text.len()
        && text.is_codepoint_boundary(at)
        && text.line_of_offset(at) == point.row)
        .then_some(at)
}
fn cancelled(flag: &AtomicUsize) -> Result<(), Error> {
    if flag.load(Ordering::Acquire) != 0 {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
fn parse(
    text: &Rope,
    old: Option<&Tree>,
    php_only: bool,
    cancel: &AtomicUsize,
) -> Result<Tree, Error> {
    cancelled(cancel)?;
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language(php_only))
        .map_err(|err| Error::Parse(err.to_string()))?;
    // SAFETY: cancel outlives the parser and parse call; parser is local and
    // dropped before this borrowed flag may be released.
    unsafe {
        parser.set_cancellation_flag(Some(cancel));
    }
    let tree = parser.parse_with(
        &mut |offset, _| text.iter_chunks(offset..).next().unwrap_or("").as_bytes(),
        old,
    );
    cancelled(cancel)?;
    tree.ok_or_else(|| Error::Parse("PHP parse returned no tree".into()))
}
impl Document {
    pub fn new(
        text: Rope,
        php_only: bool,
        cancel: &AtomicUsize,
    ) -> Result<Self, Error> {
        let tree = parse(&text, None, php_only, cancel)?;
        Self::assemble(tree, text, php_only, &[], None, cancel)
    }
    pub fn text(&self) -> &Rope {
        &self.text
    }
    pub fn index(&self) -> &RegionIndex {
        &self.index
    }
    pub fn has_syntax_errors(&self) -> bool {
        self.tree.root_node().has_error()
    }
    /// Edits must be in sequential coordinates, exactly as Tree::edit expects.
    pub fn update(
        &self,
        text: Rope,
        edits: &[InputEdit],
        cancel: &AtomicUsize,
    ) -> Result<Self, Error> {
        cancelled(cancel)?;
        if edits.is_empty() {
            return if self.text.len() == text.len()
                && self.text.iter_chunks(..).eq(text.iter_chunks(..))
            {
                Ok(self.clone())
            } else {
                Self::new(text, self.php_only, cancel)
            };
        }
        let mut tree = self.tree.clone();
        let mut units = self.units.clone();
        let mut len = self.text.len();
        for edit in edits {
            if edit.start_byte > edit.old_end_byte
                || edit.old_end_byte > len
                || edit.new_end_byte < edit.start_byte
            {
                return Err(Error::InvalidEdit);
            }
            let removed = edit.old_end_byte - edit.start_byte;
            let added = edit.new_end_byte - edit.start_byte;
            len = len - removed + added;
            tree.edit(edit);
            units.retain(|unit| {
                unit.range.end <= edit.start_byte
                    || unit.range.start >= edit.old_end_byte
            });
            for unit in &mut units {
                if unit.range.start >= edit.old_end_byte {
                    shift(&mut unit.range, removed, added);
                    for region in &mut unit.regions {
                        shift(&mut region.range, removed, added);
                    }
                }
            }
        }
        if len != text.len() {
            return Err(Error::InvalidEdit);
        }
        let tree = parse(&text, Some(&tree), self.php_only, cancel)?;
        Self::assemble(tree, text, self.php_only, &units, Some(edits), cancel)
    }
    fn assemble(
        tree: Tree,
        text: Rope,
        php_only: bool,
        old: &[Unit],
        edits: Option<&[InputEdit]>,
        cancel: &AtomicUsize,
    ) -> Result<Self, Error> {
        let mut units = Vec::new();
        let mut rescanned = 0;
        let mut host = Host::Text;
        let mut cursor = tree.root_node().walk();
        for node in tree.root_node().children(&mut cursor) {
            cancelled(cancel)?;
            let range = node.byte_range();
            // Nodes touching an edit boundary can gain/lose lexical ownership even
            // when tree-sitter reports an unchanged subtree. Never reuse those units.
            let touching = edits.is_some_and(|edits| {
                edits.iter().any(|edit| {
                    range.start <= edit.new_end_byte && range.end >= edit.start_byte
                })
            });
            if !node.has_changes() && !touching {
                if let Ok(index) =
                    old.binary_search_by_key(&range.start, |unit| unit.range.start)
                {
                    if old[index].range == range && old[index].host_before == host {
                        host = old[index].host_after.clone();
                        units.push(old[index].clone());
                        continue;
                    }
                }
            }
            let host_before = host.clone();
            let regions = detect(node, &text, php_only, &mut host, cancel)?;
            units.push(Unit {
                range,
                regions,
                host_before,
                host_after: host.clone(),
            });
            rescanned += 1;
        }
        let mut regions = Vec::new();
        let mut at = 0;
        for unit in &units {
            if at < unit.range.start {
                push(&mut regions, at..unit.range.start, Context::Php);
            }
            for region in &unit.regions {
                push(&mut regions, region.range.clone(), region.context);
            }
            at = unit.range.end;
        }
        if at < text.len() {
            push(&mut regions, at..text.len(), Context::Php);
        }
        drop(cursor);
        cancelled(cancel)?;
        let len = text.len();
        Ok(Self {
            tree,
            text,
            index: RegionIndex { regions, len },
            units,
            php_only,
            rescanned_units: rescanned,
        })
    }
    pub fn cursor_context(&self, offset: usize) -> Option<CursorContext> {
        if offset > self.text.len() || !self.text.is_codepoint_boundary(offset) {
            return None;
        }
        let context = self.index.at(offset)?;
        if context != Context::Php {
            return Some(CursorContext::NonPhp(context));
        }
        // A bounded prefix, not a full buffer copy or parse for cursor movement.
        let start = self
            .text
            .at_or_next_codepoint_boundary(offset.saturating_sub(256))
            .unwrap_or(offset);
        // Text fallbacks must not interpret keywords inside comments/strings.
        // Walk only nodes intersecting the bounded cursor window.
        let mut prefix = self.text.slice_to_cow(start..offset).into_owned();
        let mut pending = vec![self.tree.root_node()];
        let mut visited = 0;
        while let Some(node) = pending.pop() {
            visited += 1;
            if visited > 2048 {
                break;
            }
            if node.end_byte() <= start || node.start_byte() >= offset {
                continue;
            }
            if matches!(
                node.kind(),
                "comment" | "string" | "encapsed_string" | "heredoc" | "nowdoc"
            ) {
                let from = node.start_byte().max(start) - start;
                let to = node.end_byte().min(offset) - start;
                prefix.replace_range(from..to, &" ".repeat(to - from));
            } else {
                let mut cursor = node.walk();
                pending
                    .extend(node.named_children(&mut cursor).filter(|n| {
                        n.end_byte() > start && n.start_byte() < offset
                    }));
            }
        }
        let trimmed = prefix.trim_end();
        let previous = self.text.prev_codepoint_offset(offset).unwrap_or(0);
        let node = self
            .tree
            .root_node()
            .descendant_for_byte_range(previous, offset);
        let mut ancestor = node;
        let mut type_position = false;
        let mut trait_use = false;
        let mut closure_use = false;
        let mut declaration_context = None;
        while let Some(node) = ancestor {
            match node.kind() {
                "anonymous_function_use_clause" => closure_use = true,
                "use_declaration" => trait_use = true,
                "base_clause" => declaration_context = Some(CursorContext::Extends),
                "class_interface_clause" => {
                    declaration_context = Some(CursorContext::Implements)
                }
                _ => {}
            }
            if matches!(
                node.kind(),
                "named_type"
                    | "union_type"
                    | "intersection_type"
                    | "optional_type"
                    | "primitive_type"
            ) {
                type_position = true;
                break;
            }
            ancestor = node.parent();
        }
        let token = trimmed
            .rsplit(|ch: char| {
                !(ch.is_alphanumeric() || ch == '_' || ch == '$' || ch == '\\')
            })
            .next()
            .unwrap_or("");
        let before = trimmed.strip_suffix(token).unwrap_or(trimmed).trim_end();
        Some(if trimmed.ends_with("->") || before.ends_with("->") {
            CursorContext::InstanceMember
        } else if trimmed.ends_with("::") || before.ends_with("::") {
            CursorContext::StaticMember
        } else if token.starts_with('$') {
            CursorContext::Variable
        } else {
            let word = if prefix.ends_with(char::is_whitespace) {
                token
            } else {
                before.split_whitespace().next_back().unwrap_or("")
            };
            let tail = trimmed.rsplit(['{', '}', ';']).next().unwrap_or(trimmed);
            let list_context = if tail
                .split(|c: char| c.is_whitespace() || c == ',')
                .any(|word| word == "implements")
            {
                Some(CursorContext::Implements)
            } else if tail
                .split(|c: char| c.is_whitespace() || c == ',')
                .any(|word| word == "extends")
            {
                Some(CursorContext::Extends)
            } else {
                None
            };
            let closure_capture = tail.rfind("use").is_some_and(|at| {
                tail[..at].contains("function")
                    && tail[..at]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_whitespace)
                    && tail[at + 3..].trim_start().starts_with('(')
            });
            if trait_use || closure_use || closure_capture {
                return Some(CursorContext::Expression);
            }
            if let Some(context) = declaration_context.or(list_context) {
                // Interfaces extend interfaces, classes extend classes.
                let mut owner = node;
                let mut interface =
                    tail.split_whitespace().any(|word| word == "interface");
                while let Some(n) = owner {
                    if n.kind() == "interface_declaration" {
                        interface = true;
                        break;
                    }
                    owner = n.parent();
                }
                return Some(if interface && context == CursorContext::Extends {
                    CursorContext::Implements
                } else {
                    context
                });
            }
            match word {
                "new" => CursorContext::New,
                "use" => CursorContext::Import,
                "extends" => CursorContext::Extends,
                "implements" => CursorContext::Implements,
                _ if type_position => CursorContext::Type,
                _ => CursorContext::Expression,
            }
        })
    }
}
fn shift(range: &mut Range<usize>, removed: usize, added: usize) {
    range.start = range.start - removed + added;
    range.end = range.end - removed + added;
}
fn push(regions: &mut Vec<LanguageRegion>, range: Range<usize>, context: Context) {
    if range.is_empty() {
        return;
    }
    if let Some(last) = regions.last_mut() {
        if last.context == context && last.range.end == range.start {
            last.range.end = range.end;
            return;
        }
    }
    regions.push(LanguageRegion { range, context });
}
fn detect(
    node: tree_sitter::Node,
    text: &Rope,
    php_only: bool,
    host: &mut Host,
    cancel: &AtomicUsize,
) -> Result<Vec<LanguageRegion>, Error> {
    static MIXED: std::sync::OnceLock<tree_sitter::Query> =
        std::sync::OnceLock::new();
    static PURE: std::sync::OnceLock<tree_sitter::Query> =
        std::sync::OnceLock::new();
    let query = if php_only { &PURE } else { &MIXED }.get_or_init(|| {
        tree_sitter::Query::new(
            &language(php_only),
            if php_only {
                include_str!("regions-only.scm")
            } else {
                include_str!("regions.scm")
            },
        )
        .expect("checked-in PHP region query")
    });
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut overlays = Vec::new();
    for (found, index) in cursor.captures(query, node, |node: tree_sitter::Node| {
        text.iter_chunks(node.byte_range()).map(str::as_bytes)
    }) {
        let current = found.captures[index].node;
        cancelled(cancel)?;
        match current.kind() {
            "text" => html(
                &text.slice_to_cow(current.byte_range()),
                current.start_byte(),
                &mut overlays,
                host,
            ),
            "comment" => {
                if text
                    .iter_chunks(current.byte_range())
                    .next()
                    .is_some_and(|chunk| chunk.starts_with("/**"))
                {
                    overlays.push(LanguageRegion {
                        range: current.byte_range(),
                        context: Context::PhpDoc,
                    });
                }
            }
            "string" | "encapsed_string" => {
                let mut cursor = current.walk();
                for child in current.named_children(&mut cursor) {
                    if child.kind() == "string_content" {
                        let content = text.slice_to_cow(child.byte_range());
                        if sql(&content) {
                            overlays.push(LanguageRegion {
                                range: child.byte_range(),
                                context: Context::Sql,
                            });
                        }
                    }
                }
            }
            "heredoc" | "nowdoc" => {
                let mut cursor = current.walk();
                let children =
                    current.named_children(&mut cursor).collect::<Vec<_>>();
                let sql_label = children
                    .iter()
                    .find(|child| child.kind() == "heredoc_start")
                    .is_some_and(|child| {
                        text.slice_to_cow(child.byte_range())
                            .eq_ignore_ascii_case("SQL")
                    });
                if sql_label {
                    for child in children {
                        if matches!(child.kind(), "heredoc_body" | "nowdoc_body") {
                            overlays.push(LanguageRegion {
                                range: child.byte_range(),
                                context: Context::Sql,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    overlays.sort_by_key(|region| region.range.start);
    let mut result = Vec::new();
    let mut at = node.start_byte();
    for region in overlays {
        if region.range.start < at {
            continue;
        }
        push(&mut result, at..region.range.start, Context::Php);
        at = region.range.end;
        push(&mut result, region.range, region.context);
    }
    push(&mut result, at..node.end_byte(), Context::Php);
    Ok(result)
}
fn sql(text: &str) -> bool {
    let first = text
        .trim_start()
        .split(|ch: char| !ch.is_ascii_alphabetic())
        .next()
        .unwrap_or("");
    matches!(
        first.to_ascii_uppercase().as_str(),
        "SELECT"
            | "INSERT"
            | "UPDATE"
            | "DELETE"
            | "CREATE"
            | "ALTER"
            | "WITH"
            | "DROP"
    )
}
fn html(source: &str, base: usize, out: &mut Vec<LanguageRegion>, host: &mut Host) {
    let lower = source.to_ascii_lowercase();
    let bytes = source.as_bytes();
    let mut at = 0;
    while at < source.len() {
        match host {
            Host::Text => {
                let end = lower[at..].find('<').map_or(source.len(), |end| at + end);
                push(out, base + at..base + end, Context::Html);
                at = end;
                if at < source.len() {
                    if lower[at..].starts_with("<!--") {
                        push(out, base + at..base + at + 4, Context::Html);
                        at += 4;
                        *host = Host::Comment;
                    } else {
                        *host = Host::Tag {
                            buffer: String::from("<"),
                            quote: None,
                        };
                        push(out, base + at..base + at + 1, Context::Html);
                        at += 1;
                    }
                }
            }
            Host::Comment => {
                let end = lower[at..]
                    .find("-->")
                    .map_or(source.len(), |end| at + end + 3);
                push(out, base + at..base + end, Context::Html);
                if lower[at..end].ends_with("-->") {
                    *host = Host::Text;
                }
                at = end;
            }
            Host::Raw(context) => {
                let tag = if *context == Context::Javascript {
                    "script"
                } else {
                    "style"
                };
                let closing = format!("</{tag}");
                let mut search = at;
                let close = loop {
                    match lower[search..].find(&closing) {
                        None => break source.len(),
                        Some(relative) => {
                            let candidate = search + relative;
                            let after = candidate + closing.len();
                            if lower.as_bytes().get(after).is_none_or(|byte| {
                                matches!(
                                    byte,
                                    b'>' | b' ' | b'\t' | b'\n' | b'\r' | b'/'
                                )
                            }) {
                                break candidate;
                            }
                            search = after;
                        }
                    }
                };
                push(out, base + at..base + close, *context);
                at = close;
                if at < source.len() {
                    *host = Host::Text;
                }
            }
            Host::Tag { buffer, quote } => {
                let start = at;
                let mut complete = false;
                while at < source.len() {
                    let byte = bytes[at];
                    if let Some(q) = *quote {
                        if byte == q {
                            *quote = None;
                        }
                    } else if matches!(byte, b'\'' | b'"') {
                        *quote = Some(byte);
                    } else if byte == b'>' {
                        at += 1;
                        complete = true;
                        break;
                    }
                    at += 1;
                }
                buffer.push_str(&source[start..at]);
                push(out, base + start..base + at, Context::Html);
                if complete {
                    let tag = buffer[1..]
                        .split(|ch: char| !ch.is_ascii_alphanumeric())
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    *host = match tag.as_str() {
                        "script" => Host::Raw(Context::Javascript),
                        "style" => Host::Raw(Context::Css),
                        _ => Host::Text,
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn doc(source: &str) -> Document {
        Document::new(Rope::from(source), false, &AtomicUsize::new(0)).unwrap()
    }
    fn insert(old: &str, at: usize, added: &str) -> (String, InputEdit) {
        let mut source = old.to_owned();
        source.insert_str(at, added);
        let old_rope = Rope::from(old);
        let new_rope = Rope::from(source.as_str());
        (
            source,
            InputEdit {
                start_byte: at,
                old_end_byte: at,
                new_end_byte: at + added.len(),
                start_position: byte_to_point(&old_rope, at).unwrap(),
                old_end_position: byte_to_point(&old_rope, at).unwrap(),
                new_end_position: byte_to_point(&new_rope, at + added.len())
                    .unwrap(),
            },
        )
    }
    #[test]
    fn host_context_survives_php_islands_and_quoted_attributes() {
        let source = "<script>let x=<?php echo $x; ?>; let after=1;</script><style>.x { width:<?php echo $w; ?>px; color:red; }</style><p title='before <?php echo $t; ?> <script>fake</script>'>end</p>";
        let document = doc(source);
        for (needle, context) in [
            ("let after", Context::Javascript),
            ("color:red", Context::Css),
            ("$x", Context::Php),
            ("$w", Context::Php),
            ("fake", Context::Html),
        ] {
            assert_eq!(
                document.index.at(source.find(needle).unwrap()),
                Some(context),
                "{needle}"
            );
        }
        let source = "<script>one</scripture>two</script>html";
        assert_eq!(
            doc(source).index.at(source.find("two").unwrap()),
            Some(Context::Javascript)
        );
        let (changed, edit) = insert(source, 1, "x");
        let updated = doc(source)
            .update(Rope::from(changed.as_str()), &[edit], &AtomicUsize::new(0))
            .unwrap();
        assert_eq!(updated.index, doc(&changed).index);
    }
    #[test]
    fn incremental_deletions_and_replacements_match_full() {
        let source = "<script>a<?php $x='λ'; ?>b</script><?php /** doc */ $sql='SELECT 1'; ?>end";
        let original = doc(source);
        let boundaries = source
            .char_indices()
            .map(|(at, _)| at)
            .chain([source.len()])
            .collect::<Vec<_>>();
        for window in boundaries.windows(2) {
            for inserted in ["", "λ\n", "?>"] {
                let (start, end) = (window[0], window[1]);
                let mut changed = source.to_owned();
                changed.replace_range(start..end, inserted);
                let old = Rope::from(source);
                let new = Rope::from(changed.as_str());
                let edit = InputEdit {
                    start_byte: start,
                    old_end_byte: end,
                    new_end_byte: start + inserted.len(),
                    start_position: byte_to_point(&old, start).unwrap(),
                    old_end_position: byte_to_point(&old, end).unwrap(),
                    new_end_position: byte_to_point(&new, start + inserted.len())
                        .unwrap(),
                };
                assert_eq!(
                    original
                        .update(new, &[edit], &AtomicUsize::new(0))
                        .unwrap()
                        .index,
                    doc(&changed).index,
                    "{start}..{end}"
                );
            }
        }
    }
    #[test]
    fn cursor_import_does_not_confuse_closure_or_trait_use() {
        for source in [
            "<?php $f=function() use ($x) {};",
            "<?php trait T { use Foo; }",
        ] {
            let offset = source.find("use ").unwrap() + 4;
            assert_ne!(
                doc(source).cursor_context(offset),
                Some(CursorContext::Import)
            );
        }
        for (source, expected) in [
            ("<?php interface X extends A, B", CursorContext::Implements),
            ("<?php class X implements A, B", CursorContext::Implements),
        ] {
            assert_eq!(doc(source).cursor_context(source.len()), Some(expected));
        }
        for source in [
            "<?php /* extends Foo */ ordinary",
            "<?php /* function() use ( */ ordinary",
            "<?php $s='implements Foo'; ordinary",
        ] {
            assert_eq!(
                doc(source).cursor_context(source.len()),
                Some(CursorContext::Expression)
            );
        }
        let quoted = "<?php $x=<<<'SQL'\nSELECT id FROM users\nSQL;";
        assert_eq!(
            doc(quoted).index.at(quoted.find("SELECT").unwrap()),
            Some(Context::Sql)
        );
    }
    #[test]
    fn mixed_languages_sql_doc_and_utf8() {
        let source = "<p>λ🙂</p><script>const x=1;</script><style>.x{color:red}</style><?php\n/** @param User $u */\n$sql='SELECT * FROM users';\n$x=<<<SQL\nSELECT 2\nSQL;\n$x=<<<'SQL'\nSELECT 3\nSQL;\n$x='ordinary'; ?>end";
        let doc = doc(source);
        for (needle, context) in [
            ("λ", Context::Html),
            ("const x", Context::Javascript),
            ("color", Context::Css),
            ("@param", Context::PhpDoc),
            ("SELECT *", Context::Sql),
            ("SELECT 2", Context::Sql),
            ("SELECT 3", Context::Sql),
            ("ordinary", Context::Php),
            ("$sql", Context::Php),
            ("end", Context::Html),
        ] {
            assert_eq!(
                doc.index.at(source.find(needle).unwrap()),
                Some(context),
                "{needle}"
            );
        }
        for region in doc.index.regions.windows(2) {
            assert_eq!(region[0].range.end, region[1].range.start);
        }
        assert_eq!(doc.index.regions.first().unwrap().range.start, 0);
        assert_eq!(doc.index.regions.last().unwrap().range.end, source.len());
    }
    #[test]
    fn pure_php_grammar_and_malformed_prefixes() {
        let cancel = AtomicUsize::new(0);
        let pure = Document::new(
            Rope::from("function f(User $u) { return $u; }"),
            true,
            &cancel,
        )
        .unwrap();
        assert!(!pure.has_syntax_errors());
        assert_eq!(pure.index.at(0), Some(Context::Php));
        let text = "<p>λ🙂</p><?php /** @var X */ $x=<<<SQL\nSELECT 1\nSQL; ?>end";
        for end in text.char_indices().map(|(at, _)| at).chain([text.len()]) {
            let _ = doc(&text[..end]);
        }
    }
    #[test]
    fn cursor_contexts_include_incomplete_expressions() {
        for (tail, expected) in [
            ("$", CursorContext::Variable),
            ("$us", CursorContext::Variable),
            ("$x->", CursorContext::InstanceMember),
            ("$x->na", CursorContext::InstanceMember),
            ("User::", CursorContext::StaticMember),
            ("User::na", CursorContext::StaticMember),
            ("new ", CursorContext::New),
            ("new Us", CursorContext::New),
            ("use ", CursorContext::Import),
            ("extends ", CursorContext::Extends),
            ("implements ", CursorContext::Implements),
        ] {
            let source = format!("<?php {tail}");
            assert_eq!(
                doc(&source).cursor_context(source.len()),
                Some(expected),
                "{tail}"
            );
        }
    }
    #[test]
    fn byte_points_use_bytes_not_characters() {
        let text = Rope::from("λ🙂\r\n$x");
        for offset in [0, 2, 6, 7, 8, 9, 10] {
            let point = byte_to_point(&text, offset).unwrap();
            assert_eq!(point_to_byte(&text, point), Some(offset));
        }
        assert_eq!(byte_to_point(&text, 6), Some(Point::new(0, 6)));
        assert!(byte_to_point(&text, 1).is_none());
        assert!(point_to_byte(&text, Point::new(0, 1)).is_none());
    }
    #[test]
    fn incremental_insertions_match_full_at_every_boundary() {
        let source = "<p>λ</p><?php\n/** note */\n$x='SELECT 1';\nfunction f(){return 1;}\n?>end";
        let original = doc(source);
        for at in source
            .char_indices()
            .map(|(at, _)| at)
            .chain([source.len()])
        {
            for added in [" ", "λ", "\n<?php ", "'", "<script>"] {
                let (new, edit) = insert(source, at, added);
                let incremental = original
                    .update(Rope::from(new.as_str()), &[edit], &AtomicUsize::new(0))
                    .unwrap();
                assert_eq!(incremental.index, doc(&new).index, "{at} {added:?}");
            }
        }
    }
    #[test]
    fn isolated_edit_reuses_other_top_level_units() {
        let source = format!(
            "<?php\n{}",
            (0..100)
                .map(|n| format!("$x{n}={n};\n"))
                .collect::<String>()
        );
        let original = doc(&source);
        let at = source.find("=50").unwrap() + 1;
        let (new, edit) = insert(&source, at, "1");
        let updated = original
            .update(Rope::from(new.as_str()), &[edit], &AtomicUsize::new(0))
            .unwrap();
        assert!(updated.rescanned_units < 5, "{}", updated.rescanned_units);
        assert_eq!(updated.index, doc(&new).index);
    }
    #[test]
    fn cancellation_and_invalid_edit_do_not_mutate_publication() {
        let original = doc("<?php $x=1;");
        let before = original.index.clone();
        assert!(matches!(
            original.update(Rope::from("changed"), &[], &AtomicUsize::new(1)),
            Err(Error::Cancelled)
        ));
        assert_eq!(original.index, before);
    }
    #[test]
    fn html_comments_and_attributes_do_not_inject_fake_tags() {
        let source = "<!-- <script>fake</script> --><p title='<script>fake</script>'>body</p>";
        let doc = doc(source);
        assert!(doc
            .index
            .regions
            .iter()
            .all(|region| region.context == Context::Html));
    }
}

/// Declaration-only scope facts for immediate completion. No inferred types.
#[derive(Default, Debug)]
pub struct CompletionScope {
    pub class: Option<String>,
    pub namespace: String,
    pub imports: std::collections::BTreeMap<String, String>,
    pub parent: Option<String>,
    pub variables: Vec<String>,
}
impl Document {
    /// Current unsaved top-level declarations, bounded AST traversal.
    pub fn completion_declarations(&self) -> Vec<(String, String, String)> {
        let mut out = vec![];
        let mut stack = vec![(self.tree.root_node(), String::new())];
        let mut visited = 0;
        while let Some((node, mut namespace)) = stack.pop() {
            visited += 1;
            if visited > 20000 {
                break;
            }
            let mut c = node.walk();
            for child in node.named_children(&mut c) {
                if child.kind() == "namespace_definition" {
                    let ns = child
                        .child_by_field_name("name")
                        .map(|n| self.text.slice_to_cow(n.byte_range()).into_owned())
                        .unwrap_or_default();
                    if let Some(body) = child.child_by_field_name("body") {
                        stack.push((body, ns));
                    } else {
                        namespace = ns;
                    }
                    continue;
                }
                let kind = match child.kind() {
                    "class_declaration" => "class",
                    "interface_declaration" => "interface",
                    "trait_declaration" => "trait",
                    "enum_declaration" => "enum",
                    "function_definition" => "function",
                    _ => continue,
                };
                if let Some(name) = child.child_by_field_name("name") {
                    let name =
                        self.text.slice_to_cow(name.byte_range()).into_owned();
                    let full = if namespace.is_empty() {
                        name.clone()
                    } else {
                        format!("{namespace}\\{name}")
                    };
                    out.push((name, full, kind.to_owned()));
                }
            }
        }
        out
    }
    pub fn completion_members(
        &self,
        offset: usize,
    ) -> Vec<(String, String, String, bool, Option<String>)> {
        let scope = self.completion_scope(offset);
        let Some(owner) = scope.class else {
            return vec![];
        };
        let mut node = self
            .tree
            .root_node()
            .descendant_for_byte_range(offset.saturating_sub(1), offset);
        let mut class = None;
        while let Some(n) = node {
            if matches!(
                n.kind(),
                "class_declaration" | "trait_declaration" | "enum_declaration"
            ) {
                class = Some(n);
                break;
            }
            // An unfinished method/member expression can make the whole class
            // an ERROR node. Reuse the class identity recovered by completion_scope
            // rather than losing earlier, valid member declarations.
            if n.is_error() {
                let mut cursor = n.walk();
                let matches_owner = n
                    .named_children(&mut cursor)
                    .find(|child| child.kind() == "name")
                    .is_some_and(|name| {
                        self.text.slice_to_cow(name.byte_range())
                            == owner.rsplit('\\').next().unwrap_or("")
                    });
                let head = self.text.slice_to_cow(
                    n.start_byte()..(n.start_byte() + 32).min(self.text.len()),
                );
                if matches_owner
                    && [
                        "class ",
                        "abstract class ",
                        "final class ",
                        "readonly class ",
                        "final readonly class ",
                        "readonly final class ",
                    ]
                    .iter()
                    .any(|prefix| head.trim_start().starts_with(prefix))
                {
                    class = Some(n);
                    break;
                }
            }
            node = n.parent();
        }
        let Some(class) = class else { return vec![] };
        let mut result = vec![];
        let mut stack = vec![class];
        let mut visited = 0;
        while let Some(n) = stack.pop() {
            visited += 1;
            if visited > 20000 {
                break;
            }
            let kind = match n.kind() {
                "method_declaration" => Some("method"),
                "property_element" | "property_promotion_parameter" => {
                    Some("property")
                }
                "const_element" | "enum_case" => Some("constant"),
                _ => None,
            };
            if let Some(kind) = kind {
                let mut c = n.walk();
                let name = n.child_by_field_name("name").or_else(|| {
                    n.named_children(&mut c)
                        .find(|n| matches!(n.kind(), "name" | "variable_name"))
                });
                if let Some(name) = name {
                    let parent = if matches!(
                        n.kind(),
                        "method_declaration"
                            | "property_promotion_parameter"
                            | "enum_case"
                    ) {
                        n
                    } else {
                        n.parent().unwrap_or(n)
                    };
                    let mut c = parent.walk();
                    let children = parent.named_children(&mut c).collect::<Vec<_>>();
                    let static_ = n.kind() == "enum_case"
                        || children.iter().any(|n| n.kind() == "static_modifier");
                    let visibility = children
                        .iter()
                        .find(|n| n.kind() == "visibility_modifier")
                        .map(|n| {
                            self.text.slice_to_cow(n.byte_range()).into_owned()
                        });
                    result.push((
                        self.text
                            .slice_to_cow(name.byte_range())
                            .trim_start_matches('&')
                            .trim_start()
                            .to_owned(),
                        owner.clone(),
                        kind.to_owned(),
                        static_,
                        visibility,
                    ));
                }
                // Only constructor parameters can declare promoted properties.
                // Do not descend into method bodies or nested anonymous classes.
                if n.kind() == "method_declaration"
                    && n.child_by_field_name("name").is_some_and(|name| {
                        self.text
                            .slice_to_cow(name.byte_range())
                            .eq_ignore_ascii_case("__construct")
                    })
                {
                    if let Some(parameters) = n.child_by_field_name("parameters") {
                        stack.push(parameters);
                    }
                }
                continue;
            }
            if n != class
                && matches!(
                    n.kind(),
                    "class_declaration" | "anonymous_class" | "function_definition"
                )
            {
                continue;
            }
            let mut c = n.walk();
            stack.extend(n.named_children(&mut c));
        }
        result
    }
    pub fn completion_allowed(&self, offset: usize) -> bool {
        if self.index.at(offset) != Some(Context::Php) {
            return false;
        }
        let mut node = self
            .tree
            .root_node()
            .descendant_for_byte_range(offset.saturating_sub(1), offset);
        while let Some(n) = node {
            if matches!(
                n.kind(),
                "comment"
                    | "string"
                    | "encapsed_string"
                    | "heredoc"
                    | "nowdoc"
                    | "shell_command_expression"
            ) {
                return false;
            }
            node = n.parent();
        }
        true
    }
    pub fn completion_scope(&self, offset: usize) -> CompletionScope {
        let mut result = CompletionScope::default();
        if offset > self.text.len() {
            return result;
        }
        let mut node = self
            .tree
            .root_node()
            .descendant_for_byte_range(offset.saturating_sub(1), offset);
        let mut scope = None;
        let mut class_node = None;
        while let Some(n) = node {
            if scope.is_none()
                && matches!(
                    n.kind(),
                    "function_definition"
                        | "method_declaration"
                        | "anonymous_function"
                        | "arrow_function"
                )
            {
                scope = Some(n);
            }
            if class_node.is_none()
                && matches!(
                    n.kind(),
                    "class_declaration"
                        | "interface_declaration"
                        | "trait_declaration"
                        | "enum_declaration"
                )
            {
                class_node = Some(n);
            }
            if n.is_error() {
                let start = self.text.slice_to_cow(
                    n.start_byte()..(n.start_byte() + 32).min(self.text.len()),
                );
                if start.trim_start().starts_with("class ")
                    || start.trim_start().starts_with("abstract class ")
                    || start.trim_start().starts_with("final class ")
                    || start.trim_start().starts_with("readonly class ")
                    || start.trim_start().starts_with("final readonly class ")
                    || start.trim_start().starts_with("readonly final class ")
                {
                    class_node = Some(n);
                    if scope.is_none() {
                        scope = Some(n);
                    }
                }
            }
            node = n.parent();
        }
        let root = self.tree.root_node();
        let mut cursor = root.walk();
        for n in root.named_children(&mut cursor) {
            if n.start_byte() > offset {
                break;
            }
            if n.kind() == "namespace_definition"
                && (n.child_by_field_name("body").is_none()
                    || n.byte_range().contains(&offset))
            {
                result.namespace = n
                    .child_by_field_name("name")
                    .map(|n| self.text.slice_to_cow(n.byte_range()).into_owned())
                    .unwrap_or_default();
            }
        }
        fn imports(
            n: tree_sitter::Node,
            text: &Rope,
            offset: usize,
            out: &mut std::collections::BTreeMap<String, String>,
        ) {
            if n.start_byte() > offset {
                return;
            }
            if matches!(
                n.kind(),
                "class_declaration" | "function_definition" | "method_declaration"
            ) {
                return;
            }
            if n.kind() == "namespace_use_clause" {
                let mut c = n.walk();
                if let Some(name) = n
                    .named_children(&mut c)
                    .find(|n| matches!(n.kind(), "name" | "qualified_name"))
                {
                    let mut target = text
                        .slice_to_cow(name.byte_range())
                        .trim_start_matches('\\')
                        .to_owned();
                    if let Some(group) =
                        n.parent().filter(|p| p.kind() == "namespace_use_group")
                    {
                        if let Some(declaration) = group.parent() {
                            let mut c = declaration.walk();
                            if let Some(prefix) = declaration
                                .named_children(&mut c)
                                .find(|n| n.kind() == "namespace_name")
                            {
                                target = format!(
                                    "{}\\{}",
                                    text.slice_to_cow(prefix.byte_range()),
                                    target
                                );
                            }
                        }
                    }
                    let alias = n
                        .child_by_field_name("alias")
                        .map(|n| text.slice_to_cow(n.byte_range()).into_owned())
                        .unwrap_or_else(|| {
                            target.rsplit('\\').next().unwrap_or("").to_owned()
                        });
                    out.insert(alias.to_lowercase(), target);
                }
                return;
            }
            let mut c = n.walk();
            for child in n.named_children(&mut c) {
                imports(child, text, offset, out);
            }
        }
        // Limit import traversal to active namespace body when bracketed.
        let mut c = root.walk();
        let body = root
            .named_children(&mut c)
            .find(|n| {
                n.kind() == "namespace_definition"
                    && n.byte_range().contains(&offset)
                    && n.child_by_field_name("body").is_some()
            })
            .and_then(|n| n.child_by_field_name("body"))
            .unwrap_or(root);
        imports(body, &self.text, offset, &mut result.imports);
        if let Some(class) = class_node {
            let mut class_cursor = class.walk();
            let class_name = if class.is_error() {
                class
                    .named_children(&mut class_cursor)
                    .find(|n| n.kind() == "name")
            } else {
                class.child_by_field_name("name")
            };
            if let Some(name) = class_name {
                let name = self.text.slice_to_cow(name.byte_range()).into_owned();
                let mut namespace = String::new();
                let root = self.tree.root_node();
                let mut c = root.walk();
                for n in root.named_children(&mut c) {
                    if n.start_byte() > class.start_byte() {
                        break;
                    }
                    if n.kind() == "namespace_definition" {
                        if n.child_by_field_name("body").is_none()
                            || n.byte_range().contains(&class.start_byte())
                        {
                            namespace = n
                                .child_by_field_name("name")
                                .map(|n| {
                                    self.text
                                        .slice_to_cow(n.byte_range())
                                        .into_owned()
                                })
                                .unwrap_or_default();
                        }
                    }
                }
                result.class = Some(if namespace.is_empty() {
                    name
                } else {
                    format!("{namespace}\\{name}")
                });
            }
        }
        if let Some(class) = class_node {
            let mut c = class.walk();
            if let Some(base) = class
                .named_children(&mut c)
                .find(|n| n.kind() == "base_clause")
            {
                let mut c = base.walk();
                if let Some(name) = base.named_children(&mut c).next() {
                    let raw = self.text.slice_to_cow(name.byte_range());
                    result.parent = Some(resolve_completion_name(
                        &raw,
                        &result.namespace,
                        &result.imports,
                    ));
                }
            }
        }
        if let Some(scope) = scope {
            let mut stack = vec![scope];
            let mut visited = 0;
            while let Some(n) = stack.pop() {
                visited += 1;
                if visited > 10000 {
                    break;
                }
                if n.start_byte() > offset {
                    continue;
                }
                if n != scope
                    && matches!(
                        n.kind(),
                        "function_definition"
                            | "anonymous_function"
                            | "arrow_function"
                            | "class_declaration"
                            | "anonymous_class"
                    )
                {
                    continue;
                }
                if n.kind() == "variable_name" {
                    let declaration = n.parent().is_some_and(|p| {
                        matches!(
                            p.kind(),
                            "simple_parameter"
                                | "variadic_parameter"
                                | "property_promotion_parameter"
                                | "anonymous_function_use_clause"
                        ) || (p.kind() == "assignment_expression"
                            && p.child_by_field_name("left") == Some(n))
                    });
                    if declaration {
                        result.variables.push(
                            self.text.slice_to_cow(n.byte_range()).into_owned(),
                        );
                    }
                }
                let mut c = n.walk();
                stack.extend(n.named_children(&mut c));
            }
        }
        if result.class.is_some() && scope.is_some() {
            result.variables.push("$this".into());
        }
        result.variables.sort();
        result.variables.dedup();
        result
    }
}

#[cfg(test)]
mod completion_scope_tests {
    use super::*;
    #[test]
    fn unsaved_promoted_properties_and_enum_cases() {
        let source = "<?php namespace App; class User {function __construct(public readonly string $name, protected int &$age, $ordinary) {} function run() {$local=1;}} enum State:string {case Ready='ready'; function run() {}}";
        let doc =
            Document::new(Rope::from(source), false, &AtomicUsize::new(0)).unwrap();
        let members = doc.completion_members(source.find("$local").unwrap());
        for (name, visibility) in [("$name", "public"), ("$age", "protected")] {
            assert!(
                members.iter().any(|m| m.0 == name
                    && m.1 == "App\\User"
                    && m.2 == "property"
                    && !m.3
                    && m.4.as_deref() == Some(visibility)),
                "{members:?}"
            );
        }
        assert!(!members
            .iter()
            .any(|m| m.0 == "$ordinary" || m.0 == "$local"));
        let members = doc.completion_members(source.rfind("run()").unwrap());
        assert!(
            members.iter().any(|m| m.0 == "Ready"
                && m.1 == "App\\State"
                && m.2 == "constant"
                && m.3),
            "{members:?}"
        );
    }
    #[test]
    fn class_local_scope_excludes_nested_and_future() {
        let source = "<?php namespace App; class User {function run($user) {$name=1; $missing; $f=function($nested){}; $this->save(); $later=1;}}";
        let offset = source.find("save").unwrap();
        let doc =
            Document::new(Rope::from(source), false, &AtomicUsize::new(0)).unwrap();
        let scope = doc.completion_scope(offset);
        assert_eq!(scope.class.as_deref(), Some("App\\User"));
        assert!(scope.variables.contains(&"$user".into()));
        assert!(scope.variables.contains(&"$name".into()));
        for name in ["$nested", "$later", "$missing"] {
            assert!(!scope.variables.contains(&name.into()), "{name}");
        }
    }
}

pub fn resolve_completion_name(
    name: &str,
    namespace: &str,
    imports: &std::collections::BTreeMap<String, String>,
) -> String {
    if name.starts_with('\\') {
        return name.trim_start_matches('\\').to_owned();
    }
    let mut parts = name.splitn(2, '\\');
    let first = parts.next().unwrap_or("");
    if let Some(target) = imports.get(&first.to_lowercase()) {
        return parts
            .next()
            .map(|tail| format!("{target}\\{tail}"))
            .unwrap_or_else(|| target.clone());
    }
    if namespace.is_empty() {
        name.to_owned()
    } else {
        format!("{namespace}\\{name}")
    }
}
#[cfg(test)]
mod completion_gate_tests {
    use super::*;
    #[test]
    fn false_positive_trigger_matrix() {
        for text in [
            "<script>obj->",
            "<p>$",
            "<?php // $",
            "<?php /* new  */",
            "<?php $s='new ';",
            "<style>.x::",
        ] {
            let d = Document::new(Rope::from(text), false, &AtomicUsize::new(0))
                .unwrap();
            let offset = if text.ends_with("';") {
                text.len() - 2
            } else {
                text.len()
            };
            assert!(!d.completion_allowed(offset), "{text}");
        }
        let s = "<?php class C {function m() {$this->";
        let d = Document::new(Rope::from(s), false, &AtomicUsize::new(0)).unwrap();
        assert!(d.completion_allowed(s.len()));
    }
    #[test]
    fn incomplete_member_owner_recovery() {
        let s =
            "<?php namespace App; class C extends Base {function f($user) {$this->";
        let d = Document::new(Rope::from(s), false, &AtomicUsize::new(0)).unwrap();

        assert_eq!(d.completion_scope(s.len()).class.as_deref(), Some("App\\C"));
        let s = "<?php namespace App; class NativeFixture { public function nativeSave() {} public function run() { $this->";
        let d = Document::new(Rope::from(s), false, &AtomicUsize::new(0)).unwrap();
        assert_eq!(
            d.completion_scope(s.len()).class.as_deref(),
            Some("App\\NativeFixture")
        );
        assert!(
            d.completion_members(s.len())
                .iter()
                .any(|m| m.0 == "nativeSave"),
            "{:?}",
            d.completion_members(s.len())
        );
    }
    #[test]
    fn incomplete_use_and_declaration_lists() {
        for source in [
            "<?php $f=function () use (",
            "<?php $f=function () use ($x, ",
        ] {
            let d = Document::new(Rope::from(source), false, &AtomicUsize::new(0))
                .unwrap();
            assert_ne!(d.cursor_context(source.len()), Some(CursorContext::Import));
        }
        for (source, want) in [
            ("<?php class C extends A, B", CursorContext::Extends),
            ("<?php class C implements A, B", CursorContext::Implements),
        ] {
            let d = Document::new(Rope::from(source), false, &AtomicUsize::new(0))
                .unwrap();
            assert_eq!(d.cursor_context(source.len()), Some(want));
        }
    }
    #[test]
    fn namespace_import_and_parent_resolution() {
        let s = "<?php namespace App; use Vendor\\Base as ParentType; use Vendor\\User; class C extends ParentType {function f() {User::x();}}";
        let d = Document::new(Rope::from(s), false, &AtomicUsize::new(0)).unwrap();
        let scope = d.completion_scope(s.find("User::").unwrap() + 6);
        assert_eq!(scope.parent.as_deref(), Some("Vendor\\Base"));
        assert_eq!(
            resolve_completion_name("User", &scope.namespace, &scope.imports),
            "Vendor\\User"
        );
    }
}
