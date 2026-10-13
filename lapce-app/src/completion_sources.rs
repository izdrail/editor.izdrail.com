//! Local completion sources blended into the popup next to LSP results:
//! imported VS Code snippets and words from the current buffer.
use std::collections::HashMap;

use lapce_vsix::snippets::Snippet as VsSnippet;
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, InsertTextFormat, Range,
    TextEdit,
};

/// Where a completion item came from. The order is the popup priority:
/// LSP, then snippets, then lexer word lists, then buffer words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum CompletionSource {
    #[default]
    Lsp,
    Snippet,
    /// Lexer word lists: language keywords, builtins, Laravel facades etc.
    Keyword,
    Word,
}

impl CompletionSource {
    /// Sort tier, lower is shown first.
    pub fn tier(self) -> u8 {
        match self {
            CompletionSource::Lsp => 0,
            CompletionSource::Snippet => 1,
            CompletionSource::Keyword => 2,
            CompletionSource::Word => 3,
        }
    }
}

/// Words are not offered below this many typed characters.
pub const MIN_WORD_INPUT: usize = 2;
const MIN_WORD_LEN: usize = 3;
const MAX_WORDS: usize = 2000;
/// Do not scan more than this many bytes of a buffer for words.
pub const MAX_SCAN_BYTES: usize = 1 << 20;

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Identifiers in `text`, most frequent first (ties: first seen first).
pub fn buffer_words(text: &str) -> Vec<String> {
    let mut count: HashMap<&str, (usize, usize)> = HashMap::new();
    let mut start: Option<usize> = None;
    let mut n = 0usize;
    let mut finish = |s: usize, e: usize, n: &mut usize| {
        let w = &text[s..e];
        if w.chars().count() >= MIN_WORD_LEN {
            let entry = count.entry(w).or_insert((0, *n));
            entry.0 += 1;
            *n += 1;
        }
    };
    for (i, c) in text.char_indices() {
        match start {
            None if is_ident_start(c) => start = Some(i),
            Some(s) if !is_ident(c) => {
                finish(s, i, &mut n);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        finish(s, text.len(), &mut n);
    }
    let mut words: Vec<(&str, (usize, usize))> = count.into_iter().collect();
    words.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.1.1.cmp(&b.1.1)));
    words
        .into_iter()
        .take(MAX_WORDS)
        .map(|(w, _)| w.to_string())
        .collect()
}

/// Keywords, builtin functions and types of a language, for the popup.
/// Only PHP has lists today (the same ones the PHP lexer highlights). In a
/// Laravel project the Laravel lexer lists are added on top.
pub fn language_items(language_id: &str, laravel: bool) -> Vec<CompletionItem> {
    if language_id != "php" {
        return Vec::new();
    }
    let lists = if laravel {
        lapce_core::lexilla::LARAVEL_WORD_LISTS
    } else {
        lapce_core::lexilla::PHP_WORD_LISTS
    };
    let kinds = [
        (0, CompletionItemKind::KEYWORD, "keyword"),
        (1, CompletionItemKind::FUNCTION, "builtin function"),
        (2, CompletionItemKind::CLASS, "builtin type"),
        (3, CompletionItemKind::CONSTANT, "builtin constant"),
        (4, CompletionItemKind::CLASS, "Laravel facade"),
        (5, CompletionItemKind::METHOD, "Laravel route method"),
        (6, CompletionItemKind::FUNCTION, "Artisan command"),
        (7, CompletionItemKind::FUNCTION, "Laravel helper"),
        (8, CompletionItemKind::METHOD, "Eloquent / query builder"),
    ];
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (i, kind, detail) in kinds {
        for w in lists.get(i).copied().unwrap_or("").split_whitespace() {
            // The popup replaces an identifier, so a word with other
            // characters (`make:model`) would be inserted wrongly.
            if !w.chars().all(is_ident) {
                continue;
            }
            // Lists overlap (`get`, `delete`, `value`): first list wins.
            if !seen.insert(w) {
                continue;
            }
            out.push(CompletionItem {
                label: w.to_string(),
                kind: Some(kind),
                detail: Some(detail.to_string()),
                insert_text: Some(w.to_string()),
                ..Default::default()
            });
        }
    }
    out
}

/// `$variable` names in PHP text, most frequent first. The label keeps the
/// `$`, but the inserted text and the filter text do not, because the word
/// being replaced never includes the `$`.
pub fn php_variable_items(text: &str) -> Vec<CompletionItem> {
    let mut count: HashMap<&str, (usize, usize)> = HashMap::new();
    let bytes = text.as_bytes();
    let mut n = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i + 1;
            let mut end = start;
            for (off, c) in text[start..].char_indices() {
                let ok = if off == 0 {
                    is_ident_start(c)
                } else {
                    is_ident(c)
                };
                if !ok {
                    break;
                }
                end = start + off + c.len_utf8();
            }
            if end > start {
                let e = count.entry(&text[start..end]).or_insert((0, n));
                e.0 += 1;
                n += 1;
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    let mut v: Vec<_> = count.into_iter().collect();
    v.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.1.1.cmp(&b.1.1)));
    v.into_iter()
        .take(MAX_WORDS)
        .filter(|(name, _)| *name != "this")
        .map(|(name, _)| CompletionItem {
            label: format!("${name}"),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("variable in file".to_string()),
            filter_text: Some(name.to_string()),
            insert_text: Some(name.to_string()),
            ..Default::default()
        })
        .collect()
}

pub fn word_items(words: &[String]) -> Vec<CompletionItem> {
    words
        .iter()
        .map(|w| CompletionItem {
            label: w.clone(),
            kind: Some(CompletionItemKind::TEXT),
            detail: Some("word in file".to_string()),
            insert_text: Some(w.clone()),
            ..Default::default()
        })
        .collect()
}

/// One item per snippet prefix. The body is re-indented to `indent` (the
/// indentation of the line being edited) and replaces `range`.
pub fn snippet_items(
    snippets: &[&VsSnippet],
    indent: &str,
    range: Range,
) -> Vec<CompletionItem> {
    let mut out = Vec::new();
    for s in snippets {
        let body = s
            .body
            .replace("\r\n", "\n")
            .replace('\n', &format!("\n{indent}"));
        for prefix in &s.prefix {
            if prefix.is_empty() {
                continue;
            }
            let detail = if s.description.is_empty() {
                s.name.clone()
            } else {
                format!("{} - {}", s.name, s.description)
            };
            out.push(CompletionItem {
                label: prefix.clone(),
                kind: Some(CompletionItemKind::SNIPPET),
                detail: Some(detail),
                filter_text: Some(prefix.clone()),
                insert_text: Some(body.clone()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range,
                    new_text: body.clone(),
                })),
                ..Default::default()
            });
        }
    }
    out
}

/// Leading whitespace of a line.
pub fn leading_indent(line: &str) -> &str {
    let end = line
        .char_indices()
        .find(|(_, c)| *c != ' ' && *c != '\t')
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    &line[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_ranked_by_frequency_and_filtered() {
        let w = buffer_words("fn foo() { foo(); bar_baz(1); foo(); ab; 12abc }");
        assert_eq!(w[0], "foo");
        assert!(w.contains(&"bar_baz".to_string()));
        assert!(!w.contains(&"ab".to_string()), "too short");
        assert!(
            !w.iter().any(|x| x.starts_with("12")),
            "digits cannot start"
        );
    }

    #[test]
    fn words_handle_unicode() {
        let w = buffer_words("let naïve_café = 1; naïve_café + über");
        assert_eq!(w[0], "naïve_café");
        assert!(w.contains(&"über".to_string()));
    }

    #[test]
    fn snippets_reindent_and_expand_prefixes() {
        let s = VsSnippet {
            name: "fn".into(),
            prefix: vec!["fn".into(), "func".into()],
            body: "fn $1() {\n\t$0\n}".into(),
            description: "function".into(),
            scope: vec![],
        };
        let r = Range::default();
        let items = snippet_items(&[&s], "    ", r);
        assert_eq!(items.len(), 2);
        let CompletionTextEdit::Edit(e) = items[0].text_edit.clone().unwrap() else {
            panic!()
        };
        assert_eq!(e.new_text, "fn $1() {\n    \t$0\n    }");
        assert_eq!(items[0].insert_text_format, Some(InsertTextFormat::SNIPPET));
        assert!(items[0].data.is_none(), "must not be sent to LSP resolve");
    }

    #[test]
    fn php_variables_drop_the_dollar_on_insert() {
        let v = php_variable_items("$a1 = $user->name; $user = $é; $$x; $1 $");
        let labels: Vec<_> = v.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels[0], "$user");
        assert!(
            labels.contains(&"$a1")
                && labels.contains(&"$é")
                && labels.contains(&"$x")
        );
        assert!(!labels.iter().any(|l| l.starts_with("$1")));
        assert_eq!(v[0].insert_text.as_deref(), Some("user"));
        assert_eq!(v[0].filter_text.as_deref(), Some("user"));
    }

    #[test]
    fn php_language_items_come_from_the_lexer_lists() {
        let items = language_items("php", false);
        let has =
            |l: &str, k| items.iter().any(|i| i.label == l && i.kind == Some(k));
        assert!(has("foreach", CompletionItemKind::KEYWORD));
        assert!(has("json_encode", CompletionItemKind::FUNCTION));
        assert!(has("string", CompletionItemKind::CLASS));
        assert!(language_items("rust", false).is_empty());
        assert!(language_items("rust", true).is_empty());
        // No Laravel words outside Laravel projects.
        assert!(
            !items
                .iter()
                .any(|i| i.label == "Route" || i.label == "orderBy")
        );
    }

    #[test]
    fn laravel_language_items_cover_every_laravel_list() {
        let items = language_items("php", true);
        let kind = |l: &str| {
            items
                .iter()
                .find(|i| i.label == l)
                .unwrap_or_else(|| panic!("missing {l}"))
        };
        assert_eq!(kind("Route").detail.as_deref(), Some("Laravel facade"));
        assert_eq!(
            kind("apiResource").detail.as_deref(),
            Some("Laravel route method")
        );
        assert_eq!(kind("migrate").detail.as_deref(), Some("Artisan command"));
        assert_eq!(kind("abort_if").detail.as_deref(), Some("Laravel helper"));
        assert_eq!(
            kind("orderBy").detail.as_deref(),
            Some("Eloquent / query builder")
        );
        // PHP words are still there.
        assert_eq!(kind("foreach").kind, Some(CompletionItemKind::KEYWORD));
        // Colon commands cannot be inserted over an identifier.
        assert!(!items.iter().any(|i| i.label.contains(':')));
        // Every label is offered once, even when two lists share a word.
        let mut labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        let n = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), n);
        assert_eq!(kind("get").detail.as_deref(), Some("Laravel route method"));
    }

    #[test]
    fn indent_of_line() {
        assert_eq!(leading_indent("  \tx = 1"), "  \t");
        assert_eq!(leading_indent("x"), "");
        assert_eq!(leading_indent("   "), "   ");
    }
}

#[cfg(test)]
mod loaded_tests {
    use lapce_vsix::install::Loaded;

    use super::*;

    #[test]
    fn imported_snippets_are_selected_by_language() {
        let mk = |p: &str| VsSnippet {
            name: p.into(),
            prefix: vec![p.into()],
            body: "x".into(),
            description: String::new(),
            scope: vec![],
        };
        let mut loaded = Loaded::default();
        loaded.snippets = vec![
            (Some("php".into()), vec![mk("foreach")]),
            (Some("rust".into()), vec![mk("impl")]),
        ];
        let items = snippet_items(&loaded.snippets_for("php"), "", Range::default());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "foreach");
    }
}

/// Profile-aware static declarations, with explicit baseline/version provenance.
/// A builtin fallback is not a resolved type or a complete project index.
pub fn profile_items(
    language_id: &str,
    profile: &lapce_core::project_profile::ProjectProfile,
    before_cursor: &str,
) -> Vec<CompletionItem> {
    let mut items = if language_id == "php" {
        language_items(language_id, profile.laravel)
    } else {
        Vec::new()
    };
    let twig_kind = if language_id == "twig" {
        lapce_core::web_templates::completion_kind(before_cursor)
    } else {
        None
    };
    if language_id == "twig" && twig_kind.is_none() {
        return items;
    }
    // Avoid API words in PHP strings/comments and member-access contexts. Static
    // global-function fallback cannot infer the object's receiver type.
    if language_id == "php"
        && !lapce_core::web_templates::php_completion_context(before_cursor)
    {
        return Vec::new();
    }
    let mut seen: std::collections::HashSet<String> =
        items.iter().map(|i| i.label.clone()).collect();
    let symbols: Vec<_> = if language_id == "php" {
        lapce_core::web_metadata::php_symbols(profile).collect()
    } else if language_id == "twig" {
        lapce_core::web_metadata::symbols()
            .iter()
            .filter(|s| s.framework == "twig" && Some(s.kind.as_str()) == twig_kind)
            .collect()
    } else {
        return items;
    };
    for symbol in symbols {
        if !seen.insert(symbol.name.clone()) {
            continue;
        }
        items.push(CompletionItem {
            label: symbol.name.clone(),
            kind: Some(if symbol.kind == "class" {
                CompletionItemKind::CLASS
            } else {
                CompletionItemKind::FUNCTION
            }),
            detail: Some(format!(
                "{} {} baseline{}",
                symbol.framework,
                symbol.version,
                symbol
                    .signature
                    .as_ref()
                    .map(|s| format!(": {s}"))
                    .unwrap_or_default()
            )),
            documentation: Some(lsp_types::Documentation::String(
                symbol.doc_url.clone(),
            )),
            insert_text: Some(symbol.name.clone()),
            ..Default::default()
        });
    }
    items
}

#[cfg(test)]
mod profile_tests {
    use super::*;
    #[test]
    fn wordpress_and_twig_sources_are_context_separate() {
        let p = lapce_core::project_profile::ProjectProfile {
            wordpress: true,
            ..Default::default()
        };
        assert!(
            profile_items("php", &p, "<?php get_")
                .iter()
                .any(|i| i.label == "get_post_meta"
                    && i.detail.as_deref().unwrap().contains("6.8.3"))
        );
        assert!(
            !profile_items("php", &p, "<?php get_")
                .iter()
                .any(|i| i.label == "upper")
        );
        assert!(profile_items("php", &p, "<?php 'get_").is_empty());
        assert!(profile_items("php", &p, "<?php $x->get_").is_empty());
        let t = profile_items("twig", &p, "{{ x|up");
        assert!(t.iter().any(|i| i.label == "upper"));
        assert!(!t.iter().any(|i| i.label == "get_post_meta"));
        assert!(profile_items("twig", &p, "<div>up").is_empty());
    }
}
