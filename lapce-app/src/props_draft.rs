//! "Insert @props Draft": in a Blade component view, builds an `@props([...])`
//! block from the attribute names that other views pass to this component
//! (`<x-forms.input type="text" :disabled="$x" />`), most used first.
//!
//! The draft lists names only, since defaults cannot be known. Framework
//! attributes (`wire:*`, `x-*`, `@click`, `class`, `style`) are left out and
//! `kebab-case` becomes `camelCase`, as Blade names the variable. Static text
//! scan, nothing is executed. Bounded: depth 8, 1500 files, 1 MB per file,
//! 2000 bytes per tag, 30 names.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

const MAX_FILE: u64 = 1 << 20;
const MAX_FILES: usize = 1500;
const MAX_DEPTH: usize = 8;
const MAX_TAG: usize = 2000;
const MAX_NAMES: usize = 30;
const MAX_TAGS_PER_FILE: usize = 2000;

/// `resources/views/components/forms/input.blade.php` -> `forms.input`
/// (`card/index.blade.php` -> `card`).
pub fn component_name(root: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(root.join("resources/views/components")).ok()?;
    let s = rel.to_str()?.strip_suffix(".blade.php")?;
    let s = s.strip_suffix("/index").unwrap_or(s);
    (!s.is_empty() && s != "index" && !s.contains("..")).then(|| s.replace('/', "."))
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_FILES {
        return;
    }
    if depth == 0 && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink()) {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() {
            walk(&p, depth + 1, out);
        } else if ft.is_file() && p.to_string_lossy().ends_with(".blade.php") {
            out.push(p);
        }
        if out.len() >= MAX_FILES {
            return;
        }
    }
}

fn camel(attr: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in attr.chars() {
        if c == '-' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Attribute names used on `<x-name ...>` tags in `text`, one entry per tag
/// occurrence of the name.
pub fn attrs_in(text: &str, name: &str) -> Vec<Vec<String>> {
    let open = format!("<x-{name}");
    let mut tags = Vec::new();
    let mut from = 0;
    while let Some(p) = text[from..].find(&open) {
        if tags.len() >= MAX_TAGS_PER_FILE {
            break;
        }
        let start = from + p + open.len();
        from = start;
        let next = text[start..].chars().next();
        if !matches!(next, Some(c) if c.is_whitespace() || c == '/' || c == '>') {
            continue; // `<x-name-longer` or `<x-name.sub`
        }
        let mut end = (start + MAX_TAG).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let body = &text[start..end];
        let mut names = Vec::new();
        let mut quote: Option<char> = None;
        let mut depth = 0usize;
        let mut word = String::new();
        let mut done = false;
        let mut flush = |word: &mut String, names: &mut Vec<String>| {
            if word.is_empty() {
                return;
            }
            let w = word.trim_start_matches(':').to_string();
            word.clear();
            let skip = w.is_empty()
                || w.starts_with('@')
                || w.starts_with("x-")
                || w.starts_with("wire:")
                || w.contains(':')
                || w.contains('.')
                || w == "class"
                || w == "style"
                || !w.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
            if !skip && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                names.push(camel(&w));
            }
        };
        for c in body.chars() {
            match quote {
                Some(q) => {
                    if c == q {
                        quote = None;
                    }
                }
                None => match c {
                    '"' | '\'' => {
                        word.clear();
                        quote = Some(c);
                    }
                    '{' => depth += 1,
                    '}' => depth = depth.saturating_sub(1),
                    '>' | '<' if depth == 0 => {
                        flush(&mut word, &mut names);
                        done = true;
                        break;
                    }
                    '=' if depth == 0 => flush(&mut word, &mut names),
                    c if c.is_whitespace() || c == '/' => {
                        if depth == 0 {
                            flush(&mut word, &mut names);
                        }
                    }
                    c if depth == 0 => word.push(c),
                    _ => {}
                },
            }
        }
        if !done {
            flush(&mut word, &mut names);
        }
        tags.push(names);
    }
    tags
}

/// Attribute names for `name` across the project's Blade views, most used
/// first, then alphabetical.
pub fn scan(root: &Path, name: &str) -> Vec<String> {
    let mut files = Vec::new();
    walk(&root.join("resources/views"), 0, &mut files);
    let mut counts: HashMap<String, usize> = HashMap::new();
    for f in files {
        let Ok(md) = std::fs::symlink_metadata(&f) else { continue };
        if !md.is_file() || md.len() > MAX_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for tag in attrs_in(&text, name) {
            let mut seen = std::collections::HashSet::new();
            for a in tag {
                if seen.insert(a.clone()) {
                    *counts.entry(a).or_default() += 1;
                }
            }
        }
    }
    let mut v: Vec<(String, usize)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.into_iter().map(|(a, _)| a).take(MAX_NAMES).collect()
}

/// The draft block, with `nl` line endings.
pub fn draft(attrs: &[String], nl: &str) -> String {
    let mut s = format!("@props([{nl}");
    for a in attrs {
        s.push_str(&format!("    '{a}',{nl}"));
    }
    s.push_str(&format!("]){nl}"));
    s
}

/// Whether the text already declares `@props`.
pub fn has_props(text: &str) -> bool {
    text.contains("@props")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let r = Path::new("/p");
        let f = |s: &str| component_name(r, &r.join(s));
        assert_eq!(f("resources/views/components/forms/input.blade.php").as_deref(), Some("forms.input"));
        assert_eq!(f("resources/views/components/card/index.blade.php").as_deref(), Some("card"));
        assert_eq!(f("resources/views/components/index.blade.php"), None);
        assert_eq!(f("resources/views/page.blade.php"), None);
        assert_eq!(f("resources/views/components/a.php"), None);
    }

    #[test]
    fn attributes() {
        let t = "<x-btn type=\"a\" :disabled=\"$x\" required wire:click=\"s\" class=\"p\" @click=\"y\" data-id=\"1\" />\n<x-btn-long a=\"1\"/>\n<x-btn\n  :max-length=\"{{ 1 }}\"\n  x-on:foo=\"1\"\n>x</x-btn>\n<x-btnx a=\"1\">";
        let tags = attrs_in(t, "btn");
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0], vec!["type", "disabled", "required", "dataId"]);
        assert_eq!(tags[1], vec!["maxLength"]);
        assert!(attrs_in("<x-btn", "btn").is_empty());
        assert_eq!(attrs_in("<x-btn a=\"1\"", "btn")[0], vec!["a"]);
        assert_eq!(attrs_in("<x-btn title=\"a > b\" c=\"d\">", "btn")[0], vec!["title", "c"]);
    }

    #[test]
    fn scan_and_draft() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::create_dir_all(r.join("resources/views/pages")).unwrap();
        std::fs::write(r.join("resources/views/pages/a.blade.php"), "<x-forms.input type=\"x\" name=\"n\"/><x-forms.input name=\"m\">").unwrap();
        std::fs::write(r.join("resources/views/pages/b.blade.php"), "<x-forms.input name=\"q\" label=\"l\"/>").unwrap();
        let v = scan(r, "forms.input");
        assert_eq!(v, vec!["name", "label", "type"]);
        assert_eq!(draft(&v, "\n"), "@props([\n    'name',\n    'label',\n    'type',\n])\n");
        assert!(scan(&r.join("nope"), "x").is_empty());
        assert!(has_props("a @props(['x'])") && !has_props("none"));
    }

    #[test]
    fn never_panics_and_is_fast() {
        for t in ["é", "<x-a", "<x-a é=\"é", "<x-a {{ ", "<x-a '", "<x-a :=\"\"", "<x-a =>"] {
            let _ = attrs_in(t, "a");
        }
        let start = std::time::Instant::now();
        let _ = attrs_in(&"<x-a b=\"c\" ".repeat(100_000), "a");
        let _ = attrs_in(&"<x-a ".repeat(200_000), "a");
        assert!(start.elapsed().as_secs() < 5);
    }
}
