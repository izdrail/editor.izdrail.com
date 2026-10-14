//! "Find Component Usages": for the Blade/Livewire component open in the
//! editor, list every place under `resources/views` that renders it
//! (`<x-name ...>`, `<livewire:name ...>`, `@livewire('name')`).
//!
//! Static text scan. Nothing is executed. Bounded: depth 8, 5000 files,
//! 1 MB per file, 500 results, symlinks not followed.
use std::path::{Path, PathBuf};

use crate::{component_index, route_index::RouteTarget};

const MAX_FILES: usize = 5000;
const MAX_FILE_BYTES: u64 = 1_000_000;
const MAX_RESULTS: usize = 500;

/// The component a file defines: `("x", "forms.input")` or
/// `("livewire", "counter")`.
pub fn component_of(root: &Path, file: &Path) -> Option<(&'static str, String)> {
    let rel_of = |base: &str| component_index::rel_of(&root.join(base), file);
    if let Some(r) = rel_of("resources/views/components") {
        return component_index::view_name(&r).map(|n| ("x", n));
    }
    if let Some(r) = rel_of("app/View/Components") {
        return component_index::class_name(&r).map(|n| ("x", n));
    }
    for base in ["app/Livewire", "app/Http/Livewire"] {
        if let Some(r) = rel_of(base) {
            return component_index::class_name(&r).map(|n| ("livewire", n));
        }
    }
    if let Some(r) = rel_of("resources/views/livewire") {
        return component_index::view_name(&r).map(|n| ("livewire", n));
    }
    None
}

fn boundary(next: Option<char>) -> bool {
    next.map_or(true, |c| c.is_whitespace() || c == '/' || c == '>')
}

/// 0-based lines in `text` that render the component.
pub fn usages_in(text: &str, kind: &str, name: &str) -> Vec<usize> {
    let tag = if kind == "x" { format!("<x-{name}") } else { format!("<livewire:{name}") };
    let dir_a = format!("@livewire('{name}'");
    let dir_b = format!("@livewire(\"{name}\"");
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let mut hit = false;
        let mut from = 0;
        while let Some(p) = line[from..].find(&tag) {
            let end = from + p + tag.len();
            if boundary(line[end..].chars().next()) {
                hit = true;
                break;
            }
            from = end;
        }
        if !hit && kind == "livewire" {
            hit = line.contains(&dir_a) || line.contains(&dir_b);
        }
        if hit {
            out.push(i);
            if out.len() >= MAX_RESULTS {
                break;
            }
        }
    }
    out
}

/// Usages of the component defined by `file`. Empty when `file` is not a
/// component.
pub fn discover(root: &Path, file: &Path) -> Vec<RouteTarget> {
    let Some((kind, name)) = component_of(root, file) else { return Vec::new() };
    let mut files: Vec<PathBuf> = Vec::new();
    component_index::walk(&root.join("resources/views"), 0, &mut files);
    let mut out = Vec::new();
    for p in files.into_iter().take(MAX_FILES) {
        let is_blade = p.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.ends_with(".blade.php"));
        if !is_blade || p == file {
            continue;
        }
        if std::fs::metadata(&p).map_or(true, |m| m.len() > MAX_FILE_BYTES) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let shown = p.strip_prefix(root).unwrap_or(&p).display().to_string();
        let lines: Vec<&str> = text.lines().collect();
        for l in usages_in(&text, kind, &name) {
            let snippet = lines.get(l).map_or("", |s| s.trim());
            let snippet: String = snippet.chars().take(80).collect();
            out.push(RouteTarget { label: format!("{shown}:{}  {snippet}", l + 1), path: p.clone(), line: l });
            if out.len() >= MAX_RESULTS {
                return out;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn component_names() {
        let r = Path::new("/p");
        assert_eq!(component_of(r, &r.join("resources/views/components/forms/input.blade.php")), Some(("x", "forms.input".into())));
        assert_eq!(component_of(r, &r.join("app/View/Components/Alert.php")), Some(("x", "alert".into())));
        assert_eq!(component_of(r, &r.join("app/Livewire/UserProfile.php")), Some(("livewire", "user-profile".into())));
        assert_eq!(component_of(r, &r.join("resources/views/livewire/counter.blade.php")), Some(("livewire", "counter".into())));
        assert_eq!(component_of(r, &r.join("app/Models/User.php")), None);
    }

    #[test]
    fn matches_whole_names_only() {
        let t = "<x-btn>\n<x-btn-lg a=1>\n<x-btn\n  a=1\n<x-btn/>\nx <x-btn class=\"a\">\n";
        assert_eq!(usages_in(t, "x", "btn"), vec![0, 2, 4, 5]);
        let l = "<livewire:counter />\n@livewire('counter')\n<livewire:counter-x>\n";
        assert_eq!(usages_in(l, "livewire", "counter"), vec![0, 1]);
    }

    #[test]
    fn finds_across_views_not_self() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/components/btn.blade.php", "<x-btn>self</x-btn>");
        put(r, "resources/views/home.blade.php", "a\n<x-btn color=\"red\">Go</x-btn>\n");
        put(r, "resources/views/other.blade.php", "<x-card/>");
        put(r, "resources/views/notes.txt", "<x-btn>");
        let got = discover(r, &r.join("resources/views/components/btn.blade.php"));
        assert_eq!(got.len(), 1);
        assert!(got[0].label.starts_with("resources/views/home.blade.php:2"));
        assert_eq!(got[0].line, 1);
        assert!(discover(r, &r.join("app/Models/User.php")).is_empty());
    }

    #[test]
    fn multibyte_and_large_are_safe() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/a.blade.php", &"é<x-btn é<x-btn>\n".repeat(100));
        put(r, "resources/views/big.blade.php", &"<x-btn>\n".repeat(200_000));
        let got = discover(r, &r.join("resources/views/components/btn.blade.php"));
        assert_eq!(got.len(), 100);
        let many = "<x-btn>\n".repeat(2000);
        assert_eq!(usages_in(&many, "x", "btn").len(), MAX_RESULTS);
    }
}
