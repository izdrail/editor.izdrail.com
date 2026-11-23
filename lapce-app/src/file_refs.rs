//! Exact quoted local file references share the existing definition gesture.
//! No search, commands, URLs or reads outside the workspace.
use std::path::{Path, PathBuf};

fn quoted_path(line: &str, col: usize) -> Option<&str> {
    if line.len() > 8192 || col > line.len() || !line.is_char_boundary(col) { return None; }
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !matches!(bytes[i], b'\'' | b'"' | b'`') { i += 1; continue; }
        let quote = bytes[i];
        let start = i + 1;
        let mut end = start;
        while end < bytes.len() && bytes[end] != quote {
            if bytes[end] == b'\\' { end = (end + 2).min(bytes.len()); } else { end += 1; }
        }
        if end == bytes.len() { return None; }
        if col >= start && col < end {
            let path = &line[start..end];
            return (!path.is_empty() && path.len() <= 512
                && !path.chars().any(|c| c.is_control() || matches!(c, '\\' | '$' | '{' | '}' | ':' | '?' | '#'))
                && !path.starts_with('/')
                && Path::new(path).extension().is_some()).then_some(path);
        }
        i = end + 1;
    }
    None
}

/// Prefer the current file's folder, then the workspace root. Canonical
/// containment rejects traversal or symlinks outside the open workspace.
pub fn resolve(root: &Path, source: &Path, line: &str, col: usize) -> Option<PathBuf> {
    let relative = quoted_path(line, col)?;
    let root = root.canonicalize().ok()?;
    let source = source.canonicalize().ok()?;
    if !source.starts_with(&root) { return None; }
    for base in [source.parent()?, root.as_path()] {
        let Ok(target) = base.join(relative).canonicalize() else { continue; };
        if target.starts_with(&root) && target.is_file() { return Some(target); }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!("gravity-file-ref-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
            std::fs::create_dir_all(root.join("src")).unwrap();
            std::fs::write(root.join("src/main.php"), "<?php").unwrap();
            Self(root)
        }
        fn source(&self) -> PathBuf { self.0.join("src/main.php") }
    }
    impl Drop for Fixture { fn drop(&mut self) { std::fs::remove_dir_all(&self.0).unwrap(); } }
    #[test]
    fn exact_local_and_root_paths_prefer_source_folder() {
        let f = Fixture::new();
        std::fs::write(f.0.join("src/a.php"), "local").unwrap();
        std::fs::write(f.0.join("a.php"), "root").unwrap();
        assert_eq!(resolve(&f.0, &f.source(), "require 'a.php';", 11), Some(f.0.join("src/a.php")));
        assert_eq!(resolve(&f.0, &f.source(), "require '../a.php';", 12), Some(f.0.join("a.php")));
        assert_eq!(resolve(&f.0, &f.source(), "require 'src/a.php';", 12), Some(f.0.join("src/a.php")));
        assert!(resolve(&f.0, &f.source(), "require 'missing.php';", 12).is_none());
    }
    #[test]
    fn only_the_clicked_literal_resolves() {
        assert_eq!(quoted_path("'a.php' + \"b.ts\"", 12), Some("b.ts"));
        assert!(quoted_path("'a.php'", 0).is_none());
        assert!(quoted_path("'a.php'", 6).is_none());
        for text in ["'/etc/passwd'", "'../outside'", "'https://a/b.php'", "'$name.php'", "'a.php?x'", "'a\\'b.php'", "'unterminated.php", "'a\nb.php'"] {
            assert!(quoted_path(text, 2).is_none(), "{text}");
        }
        assert_eq!(quoted_path("'café.php'", 4), Some("café.php"));
        assert!(quoted_path("'café.php'", 5).is_none());
        assert!(quoted_path(&format!("'{}.php'", "x".repeat(8200)), 3).is_none());
    }
    #[cfg(unix)]
    #[test]
    fn outside_symlink_and_source_are_rejected() {
        let f = Fixture::new();
        std::os::unix::fs::symlink("/etc/passwd", f.0.join("src/out.php")).unwrap();
        assert!(resolve(&f.0, &f.source(), "'out.php'", 2).is_none());
        assert!(resolve(&f.0, Path::new("/etc/passwd"), "'src/main.php'", 2).is_none());
    }
}
