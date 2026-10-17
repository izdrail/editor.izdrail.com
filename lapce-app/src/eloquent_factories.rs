//! Go to an Eloquent factory from its use: on `factory` in `Post::factory()`
//! opens `class PostFactory` in `database/factories/PostFactory.php`.
//!
//! The receiver must be a static call on a class name (`Post::factory()` or
//! `\App\Models\Post::factory()`). Only the conventional file name is tried;
//! models that override `newFactory()` or use `#[UseFactory]` are not followed.
//! Textual only; nothing is executed. Bounded: 1 MB per file.
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 1_000_000;

/// Factory file and 0-based line of its class for the `factory` call under `col`.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let (class, method) = crate::eloquent_scopes::call_at(line, col)?;
    if method != "factory" {
        return None;
    }
    let p = root.join("database/factories").join(format!("{class}Factory.php"));
    let meta = std::fs::metadata(&p).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(&p).ok()?;
    let needle = format!("class {class}Factory");
    let i = text.lines().position(|l| {
        l.find(&needle).is_some_and(|at| {
            let ok_before = l[..at].chars().next_back().is_none_or(|c| c.is_whitespace());
            let ok_after = l[at + needle.len()..].chars().next().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
            ok_before && ok_after
        })
    })?;
    Some((p, i))
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
    fn factory_opens_its_class() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let f = put(r, "database/factories/PostFactory.php", "<?php\n\nnamespace Database\\Factories;\n\nfinal class PostFactory extends Factory\n{\n}\n");
        let go = |l: &str, n: &str| resolve(r, l, at(l, n));
        assert_eq!(go("Post::factory()->count(3)->create();", "factory"), Some((f.clone(), 4)));
        assert_eq!(go("\\App\\Models\\Post::factory();", "factory"), Some((f, 4)));
        // Other methods, other receivers, missing files.
        assert_eq!(go("Post::create([])", "create"), None);
        assert_eq!(go("$post->factory()", "factory"), None);
        assert_eq!(go("Comment::factory()", "factory"), None);
        assert_eq!(go("Post::factory()", "Post"), None);
    }

    #[test]
    fn similar_class_names_do_not_match() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "database/factories/PostFactory.php", "<?php\nclass PostFactoryBase {}\n");
        assert_eq!(resolve(r, "Post::factory()", 7), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "database/factories/PostFactory.php", "<?php\nclass PostFactory {}");
        for l in ["::", "Post::", "Post::factory", "Post::é(", "É::factory(", "日本::factory()", "Post::factory(", "", "Post::factoryé()"] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), l, c);
            }
        }
    }
}
