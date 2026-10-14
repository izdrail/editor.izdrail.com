//! "Find View Usages": for the Blade view open in the editor, list every place
//! that renders or includes it - `view('users.index')`, `View::make`,
//! `Route::view`, `->view(...)` / `->markdown(...)` in mailables,
//! `@include`, `@extends`, `@each`, `@component` and friends.
//!
//! The reverse of Go to Template. A line counts when it contains the view
//! name as a complete quoted string and a view call or directive. Static text
//! scan under `app/`, `routes/`, `resources/views/` and `packages/`; nothing
//! is executed. Bounded: depth 8, 3000 files, 1 MB per file, 500 results,
//! symlinks not followed.
use std::path::{Path, PathBuf};

use crate::route_index::RouteTarget;

const MAX_DEPTH: usize = 8;
const MAX_FILES: usize = 3000;
const MAX_FILE: u64 = 1 << 20;
const MAX_RESULTS: usize = 500;

const TRIGGERS: &[&str] = &[
    "view(", "View::make(", "View::first(", "Route::view(", "->view(", "->markdown(", "markdown(", "view:",
    "@include", "@extends", "@each", "@component", "@includeIf", "@includeWhen", "@includeUnless",
    "@includeFirst", "@livewire", "response()->view(",
];

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
        if out.len() >= MAX_FILES {
            return;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() {
            if p.file_name().is_some_and(|n| n == "vendor" || n == "node_modules") {
                continue;
            }
            walk(&p, depth + 1, out);
        } else if ft.is_file() && p.extension().is_some_and(|x| x == "php") {
            out.push(p);
        }
    }
}

/// `resources/views/users/index.blade.php` -> `users.index`. Component and
/// Livewire views are not treated as plain views here.
pub fn view_name(root: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(root.join("resources/views")).ok()?.to_str()?;
    let stem = rel.strip_suffix(".blade.php")?;
    if stem.is_empty() || stem.starts_with("components/") || stem.starts_with("livewire/") {
        return None;
    }
    let stem = stem.strip_suffix("/index").unwrap_or(stem);
    Some(stem.replace('/', "."))
}

/// 0-based lines of `text` that use the view `name`.
pub fn usages_in(text: &str, name: &str) -> Vec<usize> {
    if name.is_empty() {
        return Vec::new();
    }
    let sq = format!("'{name}'");
    let dq = format!("\"{name}\"");
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if (line.contains(&sq) || line.contains(&dq)) && TRIGGERS.iter().any(|t| line.contains(t)) {
            out.push(i);
            if out.len() >= MAX_RESULTS {
                break;
            }
        }
    }
    out
}

pub fn discover(root: &Path, file: &Path) -> Vec<RouteTarget> {
    let Some(name) = view_name(root, file) else { return Vec::new() };
    let mut files = Vec::new();
    for dir in ["app", "routes", "resources/views", "packages"] {
        walk(&root.join(dir), 0, &mut files);
    }
    let mut out = Vec::new();
    for p in files {
        if p == file || std::fs::symlink_metadata(&p).map_or(true, |m| !m.is_file() || m.len() > MAX_FILE) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let lines: Vec<&str> = text.lines().collect();
        let shown = p.strip_prefix(root).unwrap_or(&p).display().to_string();
        for l in usages_in(&text, &name) {
            let snippet: String = lines.get(l).map_or("", |s| s.trim()).chars().take(80).collect();
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
    fn names() {
        let r = Path::new("/p");
        let v = |s: &str| view_name(r, &r.join(s));
        assert_eq!(v("resources/views/users/index.blade.php").as_deref(), Some("users"));
        assert_eq!(v("resources/views/users/show.blade.php").as_deref(), Some("users.show"));
        assert_eq!(v("resources/views/components/btn.blade.php"), None);
        assert_eq!(v("resources/views/livewire/x.blade.php"), None);
        assert_eq!(v("resources/views/a.php"), None);
        assert_eq!(v("app/Http/X.php"), None);
    }

    #[test]
    fn matches_whole_names_on_view_lines() {
        let t = "return view('users.show', $u);\n$x = 'users.show';\nreturn view('users.show.extra');\n@include(\"users.show\")\nRoute::view('/u', 'users.show');\n// view('users.show')\n";
        assert_eq!(usages_in(t, "users.show"), vec![0, 3, 4, 5]);
        assert!(usages_in(t, "").is_empty());
    }

    #[test]
    fn finds_across_php_and_blade_not_vendor() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/users/show.blade.php", "@include('users.show')");
        put(r, "app/Http/Controllers/U.php", "<?php\nreturn view('users.show', []);\n");
        put(r, "routes/web.php", "<?php\nRoute::view('/u', 'users.show');\n");
        put(r, "resources/views/home.blade.php", "@extends('users.show')\n");
        put(r, "vendor/x/y.php", "view('users.show')");
        put(r, "app/Other.php", "view('users.index')");
        let got = discover(r, &r.join("resources/views/users/show.blade.php"));
        let shown: Vec<_> = got.iter().map(|t| t.label.split("  ").next().unwrap().to_string()).collect();
        assert_eq!(
            shown,
            ["app/Http/Controllers/U.php:2", "routes/web.php:2", "resources/views/home.blade.php:1"]
        );
        assert_eq!(got[0].line, 1);
        assert!(discover(r, &r.join("app/Other.php")).is_empty());
    }

    #[test]
    fn caps_and_symlinks() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Many.php", &"view('a');\n".repeat(2000));
        put(r, "app/Huge.php", &format!("view('a');{}", "x".repeat(2_000_000)));
        let got = discover(r, &r.join("resources/views/a.blade.php"));
        assert_eq!(got.len(), MAX_RESULTS);
        #[cfg(unix)]
        {
            let d2 = tempfile::tempdir().unwrap();
            put(&d2.path().join("real"), "A.php", "view('a');");
            std::os::unix::fs::symlink(d2.path().join("real"), d2.path().join("app")).unwrap();
            assert!(discover(d2.path(), &d2.path().join("resources/views/a.blade.php")).is_empty());
        }
    }

    #[test]
    fn random_text_never_panics() {
        let pieces = ["view(", "'", "\"", "a.b", "@include", "é", "日", "\n", " ", ")", "->markdown(", "x"];
        let mut s = 31u64;
        for _ in 0..500 {
            let mut t = String::new();
            for _ in 0..80 {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                t.push_str(pieces[(s >> 33) as usize % pieces.len()]);
            }
            let _ = usages_in(&t, "a.b");
        }
    }
}
