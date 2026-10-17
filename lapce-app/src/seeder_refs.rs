//! Go to a seeder class from its use: on `PostSeeder` in `PostSeeder::class`
//! (typically `$this->call(PostSeeder::class)` in `DatabaseSeeder`) opens
//! `class PostSeeder` in `database/seeders/PostSeeder.php`, falling back to
//! the legacy `database/seeds/` folder.
//!
//! Only the conventional file name is tried; seeders in custom folders are
//! not followed. Textual only; nothing is executed. Bounded: 1 MB per file.
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 1_000_000;
const DIRS: &[&str] = &["database/seeders", "database/seeds"];

/// Seeder file and 0-based line of its class for the `XxxSeeder::class` under `col`.
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
    // A `XxxSeeder` class name: capitalised, longer than the suffix, bounded.
    if name.len() <= "Seeder".len()
        || name.len() > 80
        || !name.ends_with("Seeder")
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
    for d in DIRS {
        let p = root.join(d).join(format!("{name}.php"));
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let needle = format!("class {name}");
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
            return Some((p, i));
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
    fn call_opens_the_seeder_class() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let f = put(
            r,
            "database/seeders/PostSeeder.php",
            "<?php\n\nnamespace Database\\Seeders;\n\nfinal class PostSeeder extends Seeder\n{\n}\n",
        );
        let u = put(r, "database/seeders/UserSeeder.php", "<?php\nclass UserSeeder {}\n");
        assert_eq!(
            resolve(r, "$this->call(PostSeeder::class);", at("$this->call(PostSeeder::class);", "PostSeeder")),
            Some((f.clone(), 4))
        );
        let l = "$this->call([PostSeeder::class, UserSeeder::class]);";
        assert_eq!(resolve(r, l, at(l, "PostSeeder")), Some((f.clone(), 4)));
        assert_eq!(resolve(r, l, at(l, "UserSeeder")), Some((u, 1)));
        // Fully qualified name resolves to the same file.
        assert_eq!(
            resolve(r, "$this->call(\\Database\\Seeders\\PostSeeder::class);", 30),
            Some((f.clone(), 4))
        );
        // Missing file does nothing.
        assert_eq!(resolve(r, "$this->call(CommentSeeder::class);", 15), None);
    }

    #[test]
    fn legacy_seeds_folder_is_a_fallback() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let f = put(r, "database/seeds/PostSeeder.php", "<?php\nclass PostSeeder {}\n");
        assert_eq!(resolve(r, "$this->call(PostSeeder::class);", 15), Some((f, 1)));
        // database/seeders wins when both exist.
        let f2 = put(r, "database/seeders/PostSeeder.php", "<?php\n\n\nclass PostSeeder {}\n");
        assert_eq!(resolve(r, "$this->call(PostSeeder::class);", 15), Some((f2, 3)));
    }

    #[test]
    fn only_seeder_class_constants_match() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "database/seeders/PostSeeder.php", "<?php\nclass PostSeeder {}\n");
        for (l, n) in [
            ("$this->call(Post::class);", "Post"),          // no Seeder suffix
            ("PostSeeder::create();", "PostSeeder"),         // not ::class
            ("$s = new PostSeeder();", "PostSeeder"),        // no ::class
            ("PostSeeder::classification", "PostSeeder"),    // longer constant name
            ("postSeeder::class", "postSeeder"),             // not capitalised
            ("Seeder::class", "Seeder"),                     // bare suffix
            ("$this->call(OtherSeeder::class);", "OtherSeeder"), // file elsewhere
        ] {
            assert_eq!(resolve(r, l, at(l, n)), None, "{l}");
        }
    }

    #[test]
    fn similar_class_names_do_not_match() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "database/seeders/PostSeeder.php", "<?php\nclass PostSeederBase {}\n");
        assert_eq!(resolve(r, "$this->call(PostSeeder::class);", 15), None);
    }

    #[test]
    fn big_file_is_not_read() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let mut big = String::from("<?php\n");
        while big.len() < 1_100_000 {
            big.push_str("// filler\n");
        }
        big.push_str("class PostSeeder {}\n");
        put(r, "database/seeders/PostSeeder.php", &big);
        assert_eq!(resolve(r, "$this->call(PostSeeder::class);", 15), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "database/seeders/PostSeeder.php", "<?php\nclass PostSeeder {}");
        for l in [
            "::class", "PostSeeder::", "PostSeeder::cla", "ÉSeeder::class", "日本Seeder::class",
            "PostSeeder::classé", "PostSeeder ::class", "", "PostSeeder\u{301}::class",
        ] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), l, c);
            }
        }
    }
}
