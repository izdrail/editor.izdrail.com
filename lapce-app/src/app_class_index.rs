//! Index for the "Go to Middleware / Job / Event" palette: PHP classes in the
//! standard Laravel folders, labelled by kind.
//!
//! Folders: `app/Http/Middleware`, `app/Jobs`, `app/Events`,
//! `app/Listeners`, `app/Observers`, `app/Notifications`, `app/Mail`,
//! `database/seeders` (or legacy `database/seeds`), `database/factories`.
//! Static scan: the class line is found by text, nothing is executed.
//! Bounded: depth 6 per folder, 1500 files in total, 64 KB read per file,
//! 5000 entries, symlinks skipped.
use std::{
    io::Read,
    path::{Path, PathBuf},
};

use crate::route_index::RouteTarget;

const MAX_DEPTH: usize = 6;
const MAX_FILES: usize = 1500;
const MAX_ENTRIES: usize = 5000;
const HEAD_BYTES: u64 = 64 * 1024;

pub const KINDS: &[(&str, &str)] = &[
    ("Middleware", "app/Http/Middleware"),
    ("Job", "app/Jobs"),
    ("Event", "app/Events"),
    ("Listener", "app/Listeners"),
    ("Observer", "app/Observers"),
    ("Notification", "app/Notifications"),
    ("Mail", "app/Mail"),
    ("Seeder", "database/seeders"),
    ("Seeder", "database/seeds"),
    ("Factory", "database/factories"),
];

fn walk(dir: &Path, depth: usize, budget: &mut usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || *budget == 0 {
        return;
    }
    if depth == 0 && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink()) {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if *budget == 0 {
            return;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() {
            walk(&p, depth + 1, budget, out);
        } else if ft.is_file() && p.extension().is_some_and(|x| x == "php") {
            out.push(p);
            *budget -= 1;
        }
    }
}

/// `(class name, 0-based line)` of the first class/interface/trait/enum
/// declaration in `text`.
pub fn class_decl(text: &str) -> Option<(String, usize)> {
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with("//") || line.starts_with('*') || line.starts_with('#') || line.starts_with("/*") {
            continue;
        }
        let mut words = line.split_whitespace().peekable();
        while let Some(w) = words.next() {
            if matches!(w, "final" | "abstract" | "readonly") {
                continue;
            }
            if matches!(w, "class" | "interface" | "trait" | "enum") {
                let name = words.next()?;
                let name: String = name.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                return (!name.is_empty()).then_some((name, i));
            }
            break;
        }
    }
    None
}

fn head(p: &Path) -> Option<String> {
    let mut buf = Vec::new();
    std::fs::File::open(p).ok()?.take(HEAD_BYTES).read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

pub fn discover(root: &Path) -> Vec<RouteTarget> {
    let mut budget = MAX_FILES;
    let mut out = Vec::new();
    for (kind, dir) in KINDS {
        let mut files = Vec::new();
        walk(&root.join(dir), 0, &mut budget, &mut files);
        for f in files {
            let (name, line) = head(&f)
                .and_then(|t| class_decl(&t))
                .unwrap_or_else(|| (f.file_stem().and_then(|s| s.to_str()).unwrap_or("?").to_string(), 0));
            let shown = f.strip_prefix(root).unwrap_or(&f).display().to_string();
            out.push(RouteTarget { label: format!("[{kind}] {name}  {shown}"), path: f, line });
            if out.len() >= MAX_ENTRIES {
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
    fn class_declarations() {
        assert_eq!(class_decl("<?php\n\nnamespace A;\n\nfinal class Foo extends Bar {\n"), Some(("Foo".into(), 4)));
        assert_eq!(class_decl("/**\n * class Nope\n */\nabstract class Base\n{}"), Some(("Base".into(), 3)));
        assert_eq!(class_decl("interface Contract {}"), Some(("Contract".into(), 0)));
        assert_eq!(class_decl("enum Status: string {}"), Some(("Status".into(), 0)));
        assert_eq!(class_decl("final readonly class Dto {}"), Some(("Dto".into(), 0)));
        assert_eq!(class_decl("<?php\n$class = 1;"), None);
        assert_eq!(class_decl("class "), None);
        assert_eq!(class_decl("class é"), None);
    }

    #[test]
    fn discovers_by_kind() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Http/Middleware/Authenticate.php", "<?php\nclass Authenticate {}\n");
        put(r, "app/Jobs/Billing/Charge.php", "<?php\nnamespace A;\nclass Charge implements ShouldQueue {}\n");
        put(r, "app/Events/Shipped.php", "<?php\nclass Shipped {}\n");
        put(r, "app/Listeners/Notify.php", "no class here");
        put(r, "app/Jobs/readme.txt", "x");
        let got = discover(r);
        let labels: Vec<_> = got.iter().map(|t| t.label.split("  ").next().unwrap().to_string()).collect();
        assert_eq!(labels, ["[Middleware] Authenticate", "[Job] Charge", "[Event] Shipped", "[Listener] Notify"]);
        assert_eq!(got[1].line, 2);
        assert_eq!(got[3].line, 0);
        assert!(discover(&r.join("none")).is_empty());
    }

    #[test]
    fn seeders_and_factories() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "database/seeders/DatabaseSeeder.php", "<?php\nclass DatabaseSeeder extends Seeder {}\n");
        put(r, "database/seeds/OldSeeder.php", "class OldSeeder {}");
        put(r, "database/factories/UserFactory.php", "<?php\nnamespace Database\\Factories;\nclass UserFactory extends Factory {}\n");
        put(r, "database/migrations/2020_x.php", "class X {}");
        let got = discover(r);
        let labels: Vec<_> = got.iter().map(|t| t.label.split("  ").next().unwrap().to_string()).collect();
        assert_eq!(labels, ["[Seeder] DatabaseSeeder", "[Seeder] OldSeeder", "[Factory] UserFactory"]);
        assert_eq!(got[2].line, 2);
    }

    #[test]
    fn bounds_and_symlinks() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        for i in 0..1600 {
            put(r, &format!("app/Jobs/J{i}.php"), "class J {}");
        }
        put(r, "app/Events/E.php", &"x".repeat(200_000));
        assert_eq!(discover(r).len(), MAX_FILES);
        #[cfg(unix)]
        {
            let d2 = tempfile::tempdir().unwrap();
            let real = d2.path().join("real");
            put(&real, "A.php", "class A {}");
            std::fs::create_dir_all(d2.path().join("app")).unwrap();
            std::os::unix::fs::symlink(&real, d2.path().join("app/Jobs")).unwrap();
            assert!(discover(d2.path()).is_empty());
        }
    }

    #[test]
    fn random_text_never_panics() {
        let pieces = ["class ", "final ", "abstract ", "enum ", "é", "日", "\n", "/*", "*/", " ", "A", "{", "readonly "];
        let mut s = 5u64;
        for _ in 0..500 {
            let mut t = String::new();
            for _ in 0..60 {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                t.push_str(pieces[(s >> 33) as usize % pieces.len()]);
            }
            let _ = class_decl(&t);
        }
    }
}
