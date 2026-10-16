//! Go to the other end of a Blade slot, from the name under the cursor:
//! `@stack('scripts')` opens the first `@push('scripts')` / `@prepend` /
//! `@pushOnce` / `@pushIf`, and `@push('scripts')` opens the `@stack('scripts')`
//! in a layout; `@yield('content')` opens `@section('content'`, and
//! `@section('content')` opens the `@yield('content')`.
//!
//! Textual only; nothing is executed. Files are searched under
//! `resources/views` in a stable order and the first match wins, so a slot used
//! by several views opens the first one (use "Blade Layout Map" for the full
//! list). Bounded: depth 8, 5000 files, 1 MB per file, symlinks not followed.
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 1_000_000;
const MAX_FILES: usize = 5000;
const MAX_DEPTH: usize = 8;

/// `(directive, name)` for the directive string argument under byte `col`.
fn slot_at(line: &str, col: usize) -> Option<(&str, &str)> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let i = line[..col].rfind(['\'', '"'])?;
    let q = line[i..].chars().next()?;
    let end = col + line[col..].find(q)?;
    let name = &line[i + 1..end];
    if name.is_empty()
        || name.len() > 80
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
    {
        return None;
    }
    let before = line[..i].trim_end().strip_suffix('(')?.trim_end();
    let at = before.rfind('@')?;
    let d = &before[at + 1..];
    ["stack", "push", "prepend", "pushOnce", "pushIf", "prependOnce", "yield", "section"]
        .iter()
        .find(|k| **k == d)
        .map(|k| (*k, name))
}

fn partners(d: &str) -> &'static [&'static str] {
    match d {
        "stack" => &["push", "prepend", "pushOnce", "pushIf", "prependOnce"],
        "push" | "prepend" | "pushOnce" | "pushIf" | "prependOnce" => &["stack"],
        "yield" => &["section"],
        _ => &["yield"],
    }
}

fn declares(l: &str, ds: &[&str], name: &str) -> bool {
    ds.iter().any(|d| {
        l.match_indices(&format!("@{d}")).any(|(i, m)| {
            let rest = l[i + m.len()..].trim_start();
            let Some(rest) = rest.strip_prefix('(') else { return false };
            let rest = rest.trim_start();
            let Some(q) = rest.chars().next().filter(|q| *q == '\'' || *q == '"') else {
                return false;
            };
            rest[1..].strip_prefix(name).is_some_and(|r| r.starts_with(q))
        })
    })
}

/// File and 0-based line of the opposite slot for the name under `col`.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let (d, name) = slot_at(line, col)?;
    let ds = partners(d);
    let mut stack = vec![(root.join("resources/views"), 0usize)];
    let mut seen = 0usize;
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.file_name()));
        for e in entries {
            seen += 1;
            if seen > MAX_FILES {
                return None;
            }
            let Ok(ft) = e.file_type() else { continue };
            let p = e.path();
            if ft.is_dir() {
                if depth < MAX_DEPTH {
                    stack.push((p, depth + 1));
                }
            } else if ft.is_file()
                && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".blade.php"))
                && e.metadata().is_ok_and(|m| m.len() <= MAX_FILE_BYTES)
            {
                let Ok(text) = std::fs::read_to_string(&p) else { continue };
                if let Some(i) = text.lines().position(|l| declares(l, ds, name)) {
                    return Some((p, i));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, text: &str) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    #[test]
    fn slots_open_their_partner() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let layout = put(r, "resources/views/layouts/app.blade.php", "<title>@yield('title')</title>\n@stack('scripts')\n@yield(\"content\")\n");
        let page = put(r, "resources/views/home.blade.php", "@extends('layouts.app')\n@section('content')\nhi\n@endsection\n@push('scripts')\n<script></script>\n@endpush\n");
        let go = |l: &str, n: &str| resolve(r, l, at(l, n));
        assert_eq!(go("@stack('scripts')", "scripts"), Some((page.clone(), 4)));
        assert_eq!(go("@push( 'scripts' )", "scripts"), Some((layout.clone(), 1)));
        assert_eq!(go("@yield('content')", "content"), Some((page.clone(), 1)));
        assert_eq!(go("@section(\"content\")", "content"), Some((layout, 2)));
        assert_eq!(go("@stack('missing')", "missing"), None);
    }

    #[test]
    fn other_strings_are_ignored() {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "resources/views/a.blade.php", "@stack('x')\n@push('x')\n");
        for (l, n) in [("@include('x')", "x"), ("foo('x')", "x"), ("@push($x)", "x"), ("@push('x y')", "x")] {
            assert_eq!(resolve(d.path(), l, at(l, n)), None, "{l}");
        }
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "resources/views/a.blade.php", "@stack('é')\n");
        for l in ["@stack('", "@push('é", "@yield(\"日本", "'", "", "@@@('é')", "@section('😀"] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), l, c);
            }
        }
    }
}
