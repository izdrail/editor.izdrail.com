//! "Blade Layout Map": jump between a child view and its layout.
//!
//! - Active file has `@extends('layouts.app')`: lists the layout file and
//!   every `@yield` / `@stack` / `@section` slot in it.
//! - Active file is a layout (has `@yield`/`@stack`): lists every view that
//!   `@extends` it, at the `@extends` line.
//!
//! Static text scan under `resources/views`; nothing is executed. Bounded:
//! depth 8, 5000 files, 1 MB per file, 500 results, symlinks not followed.
use std::path::{Path, PathBuf};

use crate::{component_index, route_index::RouteTarget};

const MAX_FILE_BYTES: u64 = 1_000_000;
const MAX_RESULTS: usize = 500;

/// Quoted first argument of the first `@directive(` in `line`, e.g. `@extends('layouts.app')` -> `layouts.app`.
pub fn directive_arg<'a>(line: &'a str, directive: &str) -> Option<&'a str> {
    let needle = format!("@{directive}");
    let mut from = 0;
    while let Some(p) = line[from..].find(&needle) {
        let after = from + p + needle.len();
        from = after;
        let Some(rest) = line[after..].trim_start().strip_prefix('(') else { continue };
        let rest = rest.trim_start();
        let Some(q) = rest.chars().next().filter(|c| *c == '\'' || *c == '"') else { continue };
        let body = &rest[1..];
        if let Some(end) = body.find(q) {
            return Some(&body[..end]);
        }
    }
    None
}

/// `layouts.app` -> `resources/views/layouts/app.blade.php`. Rejects names
/// that could leave the views directory.
pub fn view_path(root: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') || name.contains("::") {
        return None;
    }
    Some(root.join("resources/views").join(format!("{}.blade.php", name.replace('.', "/"))))
}

/// `resources/views/layouts/app.blade.php` -> `layouts.app`.
pub fn view_name_of(root: &Path, file: &Path) -> Option<String> {
    let rel = component_index::rel_of(&root.join("resources/views"), file)?;
    Some(rel.strip_suffix(".blade.php")?.replace('/', "."))
}

/// `(kind, name, 0-based line)` for each slot in a layout.
pub fn slots(text: &str) -> Vec<(&'static str, String, usize)> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        for (kind, d) in [("yield", "yield"), ("stack", "stack")] {
            if let Some(n) = directive_arg(line, d) {
                out.push((kind, n.to_string(), i));
            }
        }
        if out.len() >= MAX_RESULTS {
            break;
        }
    }
    out
}

fn read(p: &Path) -> Option<String> {
    if std::fs::metadata(p).map_or(true, |m| m.len() > MAX_FILE_BYTES) {
        return None;
    }
    std::fs::read_to_string(p).ok()
}

pub fn discover(root: &Path, file: &Path) -> Vec<RouteTarget> {
    let Some(text) = read(file) else { return Vec::new() };
    let shown = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    let mut out = Vec::new();

    // Child -> its layout and the layout's slots.
    let parent = text.lines().find_map(|l| directive_arg(l, "extends")).map(str::to_string);
    if let Some(layout) = parent.as_deref().and_then(|n| view_path(root, n)) {
        if let Some(lt) = read(&layout) {
            out.push(RouteTarget { label: format!("layout  {}", shown(&layout)), path: layout.clone(), line: 0 });
            for (kind, name, line) in slots(&lt) {
                out.push(RouteTarget { label: format!("@{kind}('{name}')  {}:{}", shown(&layout), line + 1), path: layout.clone(), line });
            }
        }
        return out;
    }

    // Layout -> its children.
    let Some(me) = view_name_of(root, file) else { return out };
    if slots(&text).is_empty() {
        return out;
    }
    let mut files = Vec::new();
    component_index::walk(&root.join("resources/views"), 0, &mut files);
    for p in files {
        let blade = p.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.ends_with(".blade.php"));
        if !blade || p == file {
            continue;
        }
        let Some(t) = read(&p) else { continue };
        if let Some((i, _)) = t.lines().enumerate().find(|(_, l)| directive_arg(l, "extends") == Some(me.as_str())) {
            out.push(RouteTarget { label: format!("extends  {}:{}", shown(&p), i + 1), path: p.clone(), line: i });
            if out.len() >= MAX_RESULTS {
                break;
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
    fn args_and_paths() {
        assert_eq!(directive_arg("  @extends('layouts.app')", "extends"), Some("layouts.app"));
        assert_eq!(directive_arg("@extends( \"a.b\" )", "extends"), Some("a.b"));
        assert_eq!(directive_arg("@extendsX('a')", "extends"), None);
        assert_eq!(directive_arg("@extends(", "extends"), None);
        assert_eq!(directive_arg("@extends('é", "extends"), None);
        let r = Path::new("/p");
        assert_eq!(view_path(r, "layouts.app").unwrap(), r.join("resources/views/layouts/app.blade.php"));
        assert!(view_path(r, "../x").is_none());
        assert!(view_path(r, "pkg::x").is_none());
        assert_eq!(view_name_of(r, &r.join("resources/views/layouts/app.blade.php")).as_deref(), Some("layouts.app"));
    }

    #[test]
    fn child_lists_layout_slots_and_layout_lists_children() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/layouts/app.blade.php", "<title>@yield('title')</title>\n@stack('scripts')\n@yield(\"content\")\n");
        put(r, "resources/views/home.blade.php", "@extends('layouts.app')\n@section('content')x@endsection\n");
        put(r, "resources/views/about.blade.php", "\n@extends('layouts.app')\n");
        put(r, "resources/views/other.blade.php", "@extends('layouts.admin')\n");
        let child = discover(r, &r.join("resources/views/home.blade.php"));
        let labels: Vec<_> = child.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels.len(), 4);
        assert!(labels[0].starts_with("layout  resources/views/layouts/app.blade.php"));
        assert!(labels[1].starts_with("@yield('title')"));
        assert_eq!(child[2].line, 1);
        let kids = discover(r, &r.join("resources/views/layouts/app.blade.php"));
        assert_eq!(kids.len(), 2);
        assert!(kids.iter().any(|t| t.path.ends_with("about.blade.php") && t.line == 1));
    }

    #[test]
    fn non_layout_and_missing_are_empty() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/plain.blade.php", "<p>hi</p>");
        put(r, "resources/views/lost.blade.php", "@extends('nope')");
        assert!(discover(r, &r.join("resources/views/plain.blade.php")).is_empty());
        assert!(discover(r, &r.join("resources/views/lost.blade.php")).is_empty());
        assert!(discover(r, &r.join("missing.blade.php")).is_empty());
    }

    #[test]
    fn soup_never_panics() {
        let pieces = ["@extends('", "@yield(\"", "é", "'", ")", "\n", "@stack(", "x.y"];
        let mut s = 7u64;
        for _ in 0..300 {
            let mut t = String::new();
            for _ in 0..60 {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                t.push_str(pieces[(s >> 33) as usize % pieces.len()]);
            }
            let _ = slots(&t);
            for l in t.lines() {
                let _ = directive_arg(l, "extends");
            }
        }
    }
}
