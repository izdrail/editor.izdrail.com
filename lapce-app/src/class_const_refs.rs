//! Go to a PHP class from a `::class` constant: on `Post` in `Post::class`
//! (or the last segment of `\App\Models\Post::class`) opens the one file named
//! `Post.php` in the conventional Laravel and Symfony folders
//! (`app/Models`, `app/Http/Controllers`, `src/Entity`, `tests/Feature`, ...).
//!
//! Only exact, unique file names count: zero or several `Post.php` candidates
//! do nothing (normal Go to Definition runs). `self::class`, `static::class`
//! and `$x::class` are not class names and are skipped. Textual only; nothing
//! is executed. Bounded: 1 MB per file.
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 1_000_000;
const MAX_NAME: usize = 80;

/// Conventional class folders, Laravel first, then Symfony, then tests.
const DIRS: &[&str] = &[
    "app/Models", "app", "app/Http/Controllers", "app/Http/Requests",
    "app/Http/Resources", "app/Http/Middleware", "app/Jobs", "app/Events",
    "app/Listeners", "app/Observers", "app/Notifications", "app/Mail",
    "app/Policies", "app/Providers", "app/Console/Commands", "app/Services",
    "app/Actions", "app/Exceptions",
    "src/Controller", "src/Entity", "src/Service", "src/Repository", "src/Form",
    "src/Command", "src/EventListener", "src/EventSubscriber", "src/Security",
    "src/Twig", "src/Message", "src/MessageHandler", "src",
    "database/seeders", "database/seeds", "database/factories",
    "tests", "tests/Feature", "tests/Unit",
];

/// `(file, 0-based declaration line)` for the `Xxx::class` under `col`, when
/// exactly one conventional `Xxx.php` exists.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let is_id = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let s = line[..col]
        .char_indices()
        .rev()
        .find(|(_, c)| !is_id(*c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let e = line[col..]
        .char_indices()
        .find(|(_, c)| !is_id(*c))
        .map_or(line.len(), |(i, _)| col + i);
    let name = &line[s..e];
    if name.len() < 2
        || name.len() > MAX_NAME
        || !name.starts_with(|c: char| c.is_ascii_uppercase())
    {
        return None;
    }
    // Only a `::class` constant fetch.
    let after = line[e..].trim_start();
    let Some(rest) = after.strip_prefix("::class") else { return None };
    if rest.chars().next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    // Unique conventional file wins; several candidates are ambiguous.
    let mut found: Option<PathBuf> = None;
    for d in DIRS {
        let p = root.join(d).join(format!("{name}.php"));
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(p);
    }
    let p = found?;
    let text = std::fs::read_to_string(&p).ok()?;
    // `class|interface|trait|enum Post` with word boundaries; else the top.
    let mut hit = None;
    for kw in ["class", "interface", "trait", "enum"] {
        let needle = format!("{kw} {name}");
        if let Some(i) = text.lines().position(|l| {
            l.find(&needle).is_some_and(|at| {
                let ok_before = l[..at].chars().next_back().is_none_or(|c| c.is_whitespace());
                let ok_after = l[at + needle.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
                ok_before && ok_after
            })
        }) {
            hit = Some(i);
            break;
        }
    }
    Some((p, hit.unwrap_or(0)))
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
    fn class_constant_opens_its_declaration() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let f = put(r, "app/Models/Post.php", "<?php\n\nnamespace App\\Models;\n\nfinal class Post extends Model\n{\n}\n");
        assert_eq!(resolve(r, "Route::model('p', Post::class);", at("Route::model('p', Post::class);", "Post::class")), Some((f.clone(), 4)));
        // Fully qualified: the last segment drives the lookup.
        let l = "Gate::policy(\\App\\Models\\Post::class, X::class);";
        assert_eq!(resolve(r, l, at(l, "Post::class")), Some((f, 4)));
        // Interface / trait / enum declarations are found too.
        let t = put(r, "app/Contracts/HasSlug.php", "<?php\ninterface HasSlug {}\n");
        let _ = t; // app/Contracts is not a conventional folder: no match.
        assert_eq!(resolve(r, "HasSlug::class", 2), None);
        let e = put(r, "src/Security/Role.php", "<?php\n\nenum Role: string\n{\n}\n");
        assert_eq!(resolve(r, "assert(Role::class);", 9), Some((e, 2)));
    }

    #[test]
    fn several_candidates_are_ambiguous() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Models/Post.php", "<?php\nclass Post {}\n");
        put(r, "src/Entity/Post.php", "<?php\nclass Post {}\n");
        assert_eq!(resolve(r, "Post::class", 2), None);
    }

    #[test]
    fn only_class_constants_match() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Models/Post.php", "<?php\nclass Post {}\n");
        for (l, n) in [
            ("Post::create([]);", "Post"),       // not ::class
            ("new Post;", "Post"),               // no ::class
            ("$x = self::class;", "self"),       // not a class name
            ("static::class", "static"),
            ("post::class", "post"),             // not capitalised
            ("X::class", "X"),                   // one letter
            ("Post::className", "Post"),         // longer constant
            ("Missing::class", "Missing"),       // no file
        ] {
            assert_eq!(resolve(r, l, at(l, n)), None, "{l}");
        }
    }

    #[test]
    fn similar_class_names_do_not_match() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Models/Post.php", "<?php\nclass PostBase {}\n");
        // The declaration does not match, so it opens at the top of the file.
        assert_eq!(resolve(r, "Post::class", 2), Some((r.join("app/Models/Post.php"), 0)));
    }

    #[test]
    fn big_file_is_not_read() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let mut big = String::from("<?php\n");
        while big.len() < 1_100_000 {
            big.push_str("// filler\n");
        }
        big.push_str("class Post {}\n");
        put(r, "app/Models/Post.php", &big);
        assert_eq!(resolve(r, "Post::class", 2), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "app/Models/Post.php", "<?php\nclass Post {}");
        for l in [
            "::class", "Post::", "Post::cla", "É::class", "日本::class", "Post::classé",
            "Post ::class", "", "Post\u{301}::class", "App\\É::class",
        ] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), l, c);
            }
        }
    }
}
