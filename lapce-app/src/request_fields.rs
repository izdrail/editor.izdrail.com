//! Go to a form request rule from the request field name: in a PHP file that
//! declares `function rules(`, the key in `$request->input('title')`,
//! `$request->has('title')`, `request('title')` or `old('title')` opens the
//! `'title' => ...` line of the `rules()` method.
//!
//! Only simple keys (letters, digits, `_`, `-`) are followed; dotted keys
//! (`items.*.name`) and variables are not. The receiver of `->input(...)` and
//! friends is not checked, so `request()->input('title')` qualifies too. Textual only; nothing is executed.
//! Bounded: 1 MB per file, 2000 lines scanned from `rules()`.
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 1_000_000;
const MAX_RULE_LINES: usize = 2000;

/// `$request->` methods whose first string argument is a field name.
const METHODS: &[&str] = &[
    "input", "query", "post", "string", "boolean", "integer", "float", "date",
    "has", "missing", "filled", "exists", "only", "except", "cookie", "header",
];
/// Free functions with the same meaning.
const FUNCTIONS: &[&str] = &["request", "old"];

/// The quoted key under `col` when it is the argument of a known call.
fn key_at(line: &str, col: usize) -> Option<&str> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    for q in ['\'', '"'] {
        for (i, _) in line.match_indices(q) {
            let start = i + 1;
            let Some(len) = line[start..].find(q) else { break };
            let end = start + len;
            if col < start || col > end {
                continue;
            }
            let key = &line[start..end];
            let ok = !key.is_empty()
                && key.len() <= 80
                && key.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'));
            if !ok {
                return None;
            }
            // The call must be `$request->method('...')` or `request/old('...')`.
            let before = line[..i].trim_end();
            let before = before.strip_suffix('(')?;
            for f in FUNCTIONS {
                if before.ends_with(f)
                    && before[..before.len() - f.len()]
                        .chars()
                        .next_back()
                        .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '>'))
                {
                    return Some(key);
                }
            }
            for m in METHODS {
                if before.ends_with(&format!("->{m}")) {
                    return Some(key);
                }
            }
            return None;
        }
    }
    None
}

/// `(current file, 0-based line of the rule)` for the field key under `col`.
pub fn resolve(root: &Path, path: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let _ = root;
    let key = key_at(line, col)?;
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let rules_at = lines.position(|l| l.contains("function rules("))?;
    let needle1 = format!("'{key}'");
    let needle2 = format!("\"{key}\"");
    for (off, l) in text.lines().skip(rules_at + 1).take(MAX_RULE_LINES).enumerate() {
        for needle in [&needle1, &needle2] {
            if l.find(needle).is_some_and(|at| {
                let before_ok = l[..at].chars().next_back().is_none_or(|c| {
                    c.is_whitespace() || matches!(c, '[' | '(' | ',')
                });
                let rest = l[at + needle.len()..].trim_start();
                before_ok && rest.starts_with("=>")
            }) {
                return Some((path.to_path_buf(), rules_at + 1 + off));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQ: &str = "<?php\n\nnamespace App\\Http\\Requests;\n\nclass StorePost extends FormRequest\n{\n    public function rules(): array\n    {\n        return [\n            'title' => 'required|max:255',\n            'body' => ['required', 'string'],\n        ];\n    }\n}\n";

    fn setup() -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("app/Http/Requests/StorePost.php");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, REQ).unwrap();
        (d, p)
    }

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    #[test]
    fn field_opens_its_rule() {
        let (d, p) = setup();
        let r = d.path();
        for l in [
            "$request->input('title');",
            "$request->has(\"title\");",
            "$request->boolean('title');",
            "$request->filled('title');",
            "request('title');",
            "old('title');",
            "request()->input('body');",
            "$request->only('body');",
        ] {
            let n = if l.contains("body") { "body" } else { "title" };
            // `function rules(` is on line 6, `return [` on 8, so the keys
            // land on lines 9 (`'title'`) and 10 (`'body'`).
            let want = if n == "title" { 9 } else { 10 };
            assert_eq!(resolve(r, &p, l, at(l, n)), Some((p.clone(), want)), "{l}");
        }
    }

    #[test]
    fn unknown_or_ambiguous_keys_do_nothing() {
        let (d, p) = setup();
        let r = d.path();
        for (l, n) in [
            ("$request->input('slug');", "slug"),        // no such rule
            ("$request->input($key);", "key"),           // not a string
            ("$request->input('items.name');", "items"), // dotted key
            ("$request->method('title');", "title"),     // not a field method
            ("myrequest('title');", "title"),            // longer function name
            ("$this->request('title');", "title"),       // the property, not the helper
        ] {
            assert_eq!(resolve(r, &p, l, at(l, n)), None, "{l}");
        }
    }

    #[test]
    fn file_without_rules_is_ignored() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.php");
        std::fs::write(&p, "<?php\nfunction index() {}\n").unwrap();
        assert_eq!(resolve(d.path(), &p, "$request->input('title');", 18), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let (d, p) = setup();
        for l in [
            "'", "\"", "request('", "$request->input('é');", "old('日本');", "(('))",
            "request('a", "input('title')", "", "$request->\u{301}input('title');",
        ] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), &p, l, c);
            }
        }
    }
}
