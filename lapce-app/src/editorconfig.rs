//! `.editorconfig` support: `indent_style`, `indent_size`/`tab_width` (indent
//! unit), `end_of_line`, `trim_trailing_whitespace` and `insert_final_newline`.
//!
//! The file is looked up from the opened file's folder upwards, stopping at a
//! file with `root = true`; nearer files and later sections win, `unset`
//! clears a value. Globs follow the EditorConfig spec (`*`, `**`, `?`,
//! `[seq]`, `[!seq]`, `{a,b}`, `{1..3}`); a glob without `/` matches the file
//! name at any depth. Each `.editorconfig` is read up to 64 KB, at most 24
//! folders up, and nothing else is looked at. Local files only.
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 64 * 1024;
const MAX_LEVELS: usize = 24;
const MAX_EXPANSIONS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eol {
    Lf,
    CrLf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Props {
    /// `Some(true)` for tabs, `Some(false)` for spaces.
    pub indent_tabs: Option<bool>,
    /// Columns per indent level; `None` when unset or `tab`.
    pub indent_size: Option<u8>,
    pub tab_width: Option<u8>,
    pub eol: Option<Eol>,
    pub trim_trailing_whitespace: Option<bool>,
    pub insert_final_newline: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    Tabs,
    Spaces(u8),
}

impl Props {
    pub fn is_empty(&self) -> bool {
        *self == Props::default()
    }

    /// The indent unit to use, given what auto-detection found (`current`).
    /// `None` when `.editorconfig` says nothing about indentation.
    pub fn indent(&self, current: Indent) -> Option<Indent> {
        let size = self.indent_size.or(self.tab_width).filter(|n| (1..=8).contains(n));
        match self.indent_tabs {
            Some(true) => Some(Indent::Tabs),
            Some(false) => Some(Indent::Spaces(size.unwrap_or(match current {
                Indent::Spaces(n) => n,
                Indent::Tabs => 4,
            }))),
            None => match (self.indent_size, current) {
                (Some(n), Indent::Spaces(_)) if (1..=8).contains(&n) => Some(Indent::Spaces(n)),
                _ => None,
            },
        }
    }
}

// ---- glob matching ------------------------------------------------------

#[derive(Debug, Clone)]
enum Tok {
    Lit(char),
    One,
    Star,
    DoubleStar,
    Class(Vec<(char, char)>, bool),
}

/// Expands `{a,b}` and `{1..3}` into plain alternatives (bounded).
fn expand(pat: &str) -> Vec<String> {
    let chars: Vec<char> = pat.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '{' => {
                let mut depth = 0;
                let mut commas = Vec::new();
                let mut j = i;
                let mut close = None;
                while j < chars.len() {
                    match chars[j] {
                        '\\' => j += 1,
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                close = Some(j);
                                break;
                            }
                        }
                        ',' if depth == 1 => commas.push(j),
                        _ => {}
                    }
                    j += 1;
                }
                let Some(close) = close else { i += 1; continue };
                let inner: String = chars[i + 1..close].iter().collect();
                let head: String = chars[..i].iter().collect();
                let tail: String = chars[close + 1..].iter().collect();
                let mut alts: Vec<String> = Vec::new();
                if commas.is_empty() {
                    if let Some((a, b)) = inner.split_once("..") {
                        if let (Ok(a), Ok(b)) = (a.parse::<i64>(), b.parse::<i64>()) {
                            let (lo, hi) = (a.min(b), a.max(b));
                            if hi - lo <= 100 {
                                alts = (lo..=hi).map(|n| n.to_string()).collect();
                            }
                        }
                    }
                    if alts.is_empty() {
                        // Literal braces: keep them and continue after.
                        let rest = expand(&tail);
                        return rest.into_iter().map(|r| format!("{head}\\{{{inner}\\}}{r}")).collect();
                    }
                } else {
                    let mut prev = i + 1;
                    for c in commas.iter().copied().chain(std::iter::once(close)) {
                        alts.push(chars[prev..c].iter().collect());
                        prev = c + 1;
                    }
                }
                let mut out = Vec::new();
                for a in alts {
                    for e in expand(&format!("{head}{a}{tail}")) {
                        out.push(e);
                        if out.len() >= MAX_EXPANSIONS {
                            return out;
                        }
                    }
                }
                return out;
            }
            _ => i += 1,
        }
    }
    vec![pat.to_string()]
}

fn tokenize(pat: &str) -> Vec<Tok> {
    let c: Vec<char> = pat.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '\\' if i + 1 < c.len() => {
                out.push(Tok::Lit(c[i + 1]));
                i += 2;
            }
            '*' => {
                if c.get(i + 1) == Some(&'*') {
                    out.push(Tok::DoubleStar);
                    i += 2;
                } else {
                    out.push(Tok::Star);
                    i += 1;
                }
            }
            '?' => {
                out.push(Tok::One);
                i += 1;
            }
            '[' => {
                if let Some(end) = c[i + 1..].iter().position(|x| *x == ']').map(|p| p + i + 1) {
                    let mut body = &c[i + 1..end];
                    let neg = body.first() == Some(&'!');
                    if neg {
                        body = &body[1..];
                    }
                    let mut set = Vec::new();
                    let mut k = 0;
                    while k < body.len() {
                        if k + 2 < body.len() && body[k + 1] == '-' {
                            set.push((body[k], body[k + 2]));
                            k += 3;
                        } else {
                            set.push((body[k], body[k]));
                            k += 1;
                        }
                    }
                    out.push(Tok::Class(set, neg));
                    i = end + 1;
                } else {
                    out.push(Tok::Lit('['));
                    i += 1;
                }
            }
            ch => {
                out.push(Tok::Lit(ch));
                i += 1;
            }
        }
    }
    out
}

/// NFA simulation: linear in pattern x text, no backtracking blow-up.
fn matches_tokens(toks: &[Tok], text: &str) -> bool {
    let n = toks.len();
    let mut cur = vec![false; n + 1];
    let close = |cur: &mut Vec<bool>| {
        for k in 0..n {
            if cur[k] && matches!(toks[k], Tok::Star | Tok::DoubleStar) {
                cur[k + 1] = true;
            }
        }
    };
    cur[0] = true;
    close(&mut cur);
    for ch in text.chars() {
        let mut next = vec![false; n + 1];
        for k in 0..n {
            if !cur[k] {
                continue;
            }
            match &toks[k] {
                Tok::Lit(l) => {
                    if *l == ch {
                        next[k + 1] = true;
                    }
                }
                Tok::One => {
                    if ch != '/' {
                        next[k + 1] = true;
                    }
                }
                Tok::Star => {
                    if ch != '/' {
                        next[k] = true;
                    }
                }
                Tok::DoubleStar => next[k] = true,
                Tok::Class(set, neg) => {
                    let hit = set.iter().any(|(a, b)| *a <= ch && ch <= *b);
                    if hit != *neg && ch != '/' {
                        next[k + 1] = true;
                    }
                }
            }
        }
        close(&mut next);
        cur = next;
        if !cur.iter().any(|b| *b) {
            return false;
        }
    }
    cur[n]
}

/// Does the section `glob` of a `.editorconfig` in `dir` apply to `file`?
fn section_matches(glob: &str, dir: &Path, file: &Path) -> bool {
    let has_slash = glob.contains('/');
    let target: String = if has_slash {
        let Ok(rel) = file.strip_prefix(dir) else { return false };
        rel.to_string_lossy().replace('\\', "/")
    } else {
        match file.file_name() {
            Some(n) => n.to_string_lossy().into_owned(),
            None => return false,
        }
    };
    let glob = glob.strip_prefix('/').unwrap_or(glob);
    expand(glob).iter().any(|g| {
        let g = if has_slash && !glob.starts_with("**") && !g.starts_with("**") { g.clone() } else { g.clone() };
        matches_tokens(&tokenize(&g), &target)
    })
}

// ---- parsing ------------------------------------------------------------

struct Parsed {
    root: bool,
    sections: Vec<(String, Vec<(String, String)>)>,
}

fn parse(text: &str) -> Parsed {
    let mut root = false;
    let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with(';') {
            continue;
        }
        if let Some(h) = l.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            sections.push((h.to_string(), Vec::new()));
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_string());
        match sections.last_mut() {
            Some(s) => s.1.push((k, v)),
            None => {
                if k == "root" && v.eq_ignore_ascii_case("true") {
                    root = true;
                }
            }
        }
    }
    Parsed { root, sections }
}

fn apply(props: &mut Props, key: &str, value: &str) {
    let v = value.trim().to_ascii_lowercase();
    let unset = v == "unset";
    let num = |v: &str| v.parse::<u8>().ok().filter(|n| *n > 0);
    match key {
        "indent_style" => {
            props.indent_tabs = match v.as_str() {
                "tab" => Some(true),
                "space" => Some(false),
                _ => None,
            }
        }
        "indent_size" => {
            props.indent_size = if unset || v == "tab" { None } else { num(&v) };
        }
        "tab_width" => props.tab_width = if unset { None } else { num(&v) },
        "end_of_line" => {
            props.eol = match v.as_str() {
                "lf" => Some(Eol::Lf),
                "crlf" => Some(Eol::CrLf),
                _ => None,
            }
        }
        "trim_trailing_whitespace" => {
            props.trim_trailing_whitespace = match v.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            }
        }
        "insert_final_newline" => {
            props.insert_final_newline = match v.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            }
        }
        _ => {}
    }
}

/// Properties from one `.editorconfig` text, applied over `props`.
fn apply_file(props: &mut Props, text: &str, dir: &Path, file: &Path) {
    for (glob, entries) in parse(text).sections {
        if section_matches(&glob, dir, file) {
            for (k, v) in entries {
                apply(props, &k, &v);
            }
        }
    }
}

/// The `.editorconfig` properties that apply to `file` (empty when none).
pub fn resolve(file: &Path) -> Props {
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    let mut dir = file.parent();
    for _ in 0..MAX_LEVELS {
        let Some(d) = dir else { break };
        let p = d.join(".editorconfig");
        if let Ok(meta) = std::fs::metadata(&p) {
            if meta.is_file() && meta.len() <= MAX_FILE_BYTES {
                if let Ok(text) = std::fs::read_to_string(&p) {
                    let root = parse(&text).root;
                    files.push((d.to_path_buf(), text));
                    if root {
                        break;
                    }
                }
            }
        }
        dir = d.parent();
    }
    let mut props = Props::default();
    for (d, text) in files.iter().rev() {
        apply_file(&mut props, text, d, file);
    }
    props
}

// ---- save-time edits ----------------------------------------------------

/// Byte-range replacements `(start, end, text)`, sorted and non-overlapping,
/// that apply `trim_trailing_whitespace` and `insert_final_newline` to `text`.
pub fn save_edits(text: &str, props: &Props) -> Vec<(usize, usize, String)> {
    let mut edits = Vec::new();
    if props.trim_trailing_whitespace == Some(true) {
        let mut pos = 0;
        for line in text.split_inclusive('\n') {
            let body = line.strip_suffix('\n').unwrap_or(line);
            let body = body.strip_suffix('\r').unwrap_or(body);
            let trimmed = body.trim_end_matches([' ', '\t']);
            if trimmed.len() != body.len() {
                edits.push((pos + trimmed.len(), pos + body.len(), String::new()));
            }
            pos += line.len();
        }
    }
    match props.insert_final_newline {
        Some(true) if !text.is_empty() && !text.ends_with('\n') => {
            edits.push((text.len(), text.len(), "\n".to_string()));
        }
        Some(false) if text.ends_with('\n') => {
            let cut = if text.ends_with("\r\n") { 2 } else { 1 };
            // A blank last line that trimming already emptied is left to it.
            let start = text.len() - cut;
            if !edits.iter().any(|(s, e, _)| *s <= start && *e >= text.len()) {
                edits.push((start, text.len(), String::new()));
            }
        }
        _ => {}
    }
    edits.sort_by_key(|e| (e.0, e.1));
    edits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(text: &str, file: &str) -> Props {
        let mut props = Props::default();
        apply_file(&mut props, text, Path::new("/proj"), Path::new(file));
        props
    }

    #[test]
    fn sections_and_precedence() {
        let t = "root = true\n[*]\nindent_style = space\nindent_size = 4\ntrim_trailing_whitespace = true\n\n[*.{js,ts}]\nindent_size = 2\n\n[Makefile]\nindent_style = tab\n\n[*.md]\ntrim_trailing_whitespace = false\ninsert_final_newline = unset\n";
        let php = p(t, "/proj/app/User.php");
        assert_eq!(php.indent_tabs, Some(false));
        assert_eq!(php.indent_size, Some(4));
        assert_eq!(php.trim_trailing_whitespace, Some(true));
        assert_eq!(p(t, "/proj/a.ts").indent_size, Some(2));
        assert_eq!(p(t, "/proj/Makefile").indent_tabs, Some(true));
        assert_eq!(p(t, "/proj/x/README.md").trim_trailing_whitespace, Some(false));
        // Values are case-insensitive; bad values are ignored.
        assert_eq!(p("[*]\nINDENT_STYLE = Tab\nindent_size = banana\n", "/proj/a").indent_tabs, Some(true));
        assert_eq!(p("[*]\nindent_size = banana\n", "/proj/a").indent_size, None);
        // `indent_size = tab` clears the number; `unset` clears too.
        assert_eq!(p("[*]\nindent_size = 4\n[*.php]\nindent_size = tab\n", "/proj/a.php").indent_size, None);
        assert_eq!(p("[*]\nindent_size = 4\n[*.php]\nindent_size = unset\n", "/proj/a.php").indent_size, None);
    }

    #[test]
    fn globs() {
        let m = |g: &str, f: &str| section_matches(g, Path::new("/proj"), Path::new(f));
        assert!(m("*", "/proj/a/b.c"));
        assert!(m("*.php", "/proj/deep/er/a.php"));
        assert!(!m("*.php", "/proj/a.phps"));
        assert!(m("*.{php,blade.php}", "/proj/v/a.blade.php"));
        assert!(m("*.min.*", "/proj/a.min.js"));
        assert!(m("{package.json,.travis.yml}", "/proj/x/package.json"));
        assert!(m("lib/**.js", "/proj/lib/a/b.js"));
        assert!(!m("lib/*.js", "/proj/lib/a/b.js"));
        assert!(m("lib/*.js", "/proj/lib/b.js"));
        assert!(m("/src/*.php", "/proj/src/a.php"));
        assert!(!m("/src/*.php", "/proj/other/src/a.php"));
        assert!(m("file[1-3].txt", "/proj/file2.txt"));
        assert!(!m("file[!1-3].txt", "/proj/file2.txt"));
        assert!(m("f?le.txt", "/proj/file.txt"));
        assert!(m("file{1..3}.txt", "/proj/file3.txt"));
        assert!(!m("file{1..3}.txt", "/proj/file4.txt"));
        assert!(m("{a,{b,c}}.x", "/proj/c.x"));
        // Unbalanced pieces are literal, and hostile patterns stay fast.
        assert!(!m("{unclosed", "/proj/a"));
        assert!(!m("[abc", "/proj/a"));
        let t = std::time::Instant::now();
        assert!(!m(&"*a".repeat(60), &format!("/proj/{}", "a".repeat(5000) + "b")));
        assert!(t.elapsed().as_millis() < 500);
    }

    #[test]
    fn indent_resolution() {
        let spaces = Props { indent_tabs: Some(false), indent_size: Some(2), ..Default::default() };
        assert_eq!(spaces.indent(Indent::Tabs), Some(Indent::Spaces(2)));
        let tabs = Props { indent_tabs: Some(true), indent_size: Some(4), ..Default::default() };
        assert_eq!(tabs.indent(Indent::Spaces(2)), Some(Indent::Tabs));
        // Style without a size keeps the detected width.
        let style_only = Props { indent_tabs: Some(false), ..Default::default() };
        assert_eq!(style_only.indent(Indent::Spaces(2)), Some(Indent::Spaces(2)));
        assert_eq!(style_only.indent(Indent::Tabs), Some(Indent::Spaces(4)));
        // Size alone only changes space indentation.
        let size_only = Props { indent_size: Some(2), ..Default::default() };
        assert_eq!(size_only.indent(Indent::Spaces(4)), Some(Indent::Spaces(2)));
        assert_eq!(size_only.indent(Indent::Tabs), None);
        assert_eq!(Props::default().indent(Indent::Tabs), None);
        // Out-of-range sizes are ignored.
        let big = Props { indent_tabs: Some(false), indent_size: Some(40), ..Default::default() };
        assert_eq!(big.indent(Indent::Spaces(3)), Some(Indent::Spaces(3)));
    }

    #[test]
    fn nearest_file_wins_and_root_stops_the_search() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::create_dir_all(r.join("a/b")).unwrap();
        std::fs::write(r.join(".editorconfig"), "[*]\nindent_style = tab\ntrim_trailing_whitespace = true\n").unwrap();
        std::fs::write(r.join("a/.editorconfig"), "[*.php]\nindent_style = space\nindent_size = 2\n").unwrap();
        let f = r.join("a/b/x.php");
        let props = resolve(&f);
        assert_eq!(props.indent_tabs, Some(false));
        assert_eq!(props.indent_size, Some(2));
        assert_eq!(props.trim_trailing_whitespace, Some(true));
        std::fs::write(r.join("a/.editorconfig"), "root = true\n[*.php]\nindent_size = 2\n").unwrap();
        let props = resolve(&f);
        assert_eq!(props.indent_tabs, None);
        assert_eq!(props.trim_trailing_whitespace, None);
        assert!(resolve(&r.join("a/b/none.txt")).indent_size.is_none());
        // Oversized and unreadable files are ignored.
        std::fs::write(r.join("a/.editorconfig"), format!("[*]\nindent_size = 2\n{}", "#".repeat(70_000))).unwrap();
        assert_eq!(resolve(&f).indent_size, None);
    }

    #[test]
    fn save_edits_trim_and_final_newline() {
        let on = Props { trim_trailing_whitespace: Some(true), insert_final_newline: Some(true), ..Default::default() };
        let t = "a  \n\tb\t \r\nc";
        let e = save_edits(t, &on);
        assert_eq!(e, vec![(1, 3, String::new()), (6, 8, String::new()), (t.len(), t.len(), "\n".into())]);
        // Apply them back to back from the end.
        let mut s = t.to_string();
        for (a, b, r) in e.iter().rev() {
            s.replace_range(*a..*b, r);
        }
        assert_eq!(s, "a\n\tb\r\nc\n");
        // Nothing to do on clean text; empty files stay empty.
        assert!(save_edits("a\nb\n", &on).is_empty());
        assert!(save_edits("", &on).is_empty());
        // insert_final_newline = false removes exactly one line break.
        let off = Props { insert_final_newline: Some(false), ..Default::default() };
        assert_eq!(save_edits("a\r\n", &off), vec![(1, 3, String::new())]);
        assert_eq!(save_edits("a\n\n", &off), vec![(2, 3, String::new())]);
        assert!(save_edits("a", &off).is_empty());
        // Trim off: whitespace is left alone.
        let keep = Props { trim_trailing_whitespace: Some(false), ..Default::default() };
        assert!(save_edits("a  \n", &keep).is_empty());
    }
}
