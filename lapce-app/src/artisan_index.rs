//! Index for the "Go to Artisan Command" palette: custom console commands.
//!
//! Sources, all by text scan (nothing is run):
//! - `app/Console/Commands/**.php`: `$signature = '...'` (the command name is
//!   its first word), `$description = '...'`, or `#[AsCommand(name: '...')]`
//!   / `$name = '...'`.
//! - `routes/console.php`: `Artisan::command('name {arg}', ...)`.
//!
//! Bounded: depth 6, 1000 files, 256 KB read per file, 2000 entries,
//! symlinks skipped.
use std::{
    io::Read,
    path::{Path, PathBuf},
};

use crate::route_index::RouteTarget;

const MAX_DEPTH: usize = 6;
const MAX_FILES: usize = 1000;
const MAX_ENTRIES: usize = 2000;
const HEAD_BYTES: u64 = 256 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub struct Cmd {
    pub name: String,
    pub description: String,
    pub line: usize,
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
        if out.len() >= MAX_FILES {
            return;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() {
            walk(&p, depth + 1, out);
        } else if ft.is_file() && p.extension().is_some_and(|x| x == "php") {
            out.push(p);
        }
    }
}

/// First quoted string in `s` (single or double quotes).
fn quoted(s: &str) -> Option<&str> {
    let q = s.find(|c| c == '\'' || c == '"')?;
    let qc = s[q..].chars().next()?;
    let body = &s[q + 1..];
    let end = body.find(qc)?;
    Some(&body[..end])
}

/// Value after `key` and an `=` / `:` / `(` on the same line.
fn value_after<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let i = line.find(key)?;
    let rest = line[i + key.len()..].trim_start();
    let rest = rest.strip_prefix('=').or_else(|| rest.strip_prefix(':')).or_else(|| rest.strip_prefix('('))?;
    quoted(rest)
}

fn first_word(sig: &str) -> &str {
    sig.split_whitespace().next().unwrap_or("")
}

/// Commands declared in a class file.
pub fn parse_class(text: &str) -> Option<Cmd> {
    let mut name: Option<(String, usize)> = None;
    let mut description = String::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with("//") || line.starts_with('*') || line.starts_with("/*") {
            continue;
        }
        if name.is_none() {
            if let Some(v) = value_after(line, "$signature") {
                if !first_word(v).is_empty() {
                    name = Some((first_word(v).to_string(), i));
                    continue;
                }
            }
            if line.contains("AsCommand") {
                if let Some(v) = value_after(line, "name") {
                    name = Some((v.to_string(), i));
                    continue;
                }
            }
            if let Some(v) = value_after(line, "$name") {
                if !v.is_empty() {
                    name = Some((v.to_string(), i));
                    continue;
                }
            }
        }
        if description.is_empty() {
            if let Some(v) = value_after(line, "$description") {
                description = v.chars().take(80).collect();
            }
        }
    }
    name.map(|(name, line)| Cmd { name, description, line })
}

/// `Artisan::command('name {arg}', ...)` entries in `routes/console.php`.
pub fn parse_closures(text: &str) -> Vec<Cmd> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        if let Some(p) = raw.find("Artisan::command(") {
            if let Some(v) = quoted(&raw[p + 17..]) {
                if !first_word(v).is_empty() {
                    out.push(Cmd { name: first_word(v).to_string(), description: String::new(), line: i });
                }
            }
        }
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    out
}

fn head(p: &Path) -> Option<String> {
    let mut buf = Vec::new();
    std::fs::File::open(p).ok()?.take(HEAD_BYTES).read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

pub fn discover(root: &Path) -> Vec<RouteTarget> {
    let mut out = Vec::new();
    let mut files = Vec::new();
    walk(&root.join("app/Console/Commands"), 0, &mut files);
    let shown = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    for f in files {
        let Some(c) = head(&f).and_then(|t| parse_class(&t)) else { continue };
        let desc = if c.description.is_empty() { String::new() } else { format!("  {}", c.description) };
        out.push(RouteTarget { label: format!("{}{desc}  {}", c.name, shown(&f)), path: f, line: c.line });
        if out.len() >= MAX_ENTRIES {
            return out;
        }
    }
    let console = root.join("routes/console.php");
    if std::fs::symlink_metadata(&console).map_or(false, |m| m.is_file()) {
        if let Some(t) = head(&console) {
            for c in parse_closures(&t) {
                out.push(RouteTarget { label: format!("{}  {}", c.name, shown(&console)), path: console.clone(), line: c.line });
                if out.len() >= MAX_ENTRIES {
                    break;
                }
            }
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
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
    fn class_forms() {
        let a = "class A extends Command {\n    protected $signature = 'mail:send {user} {--queue}';\n    protected $description = 'Send the mail';\n}";
        assert_eq!(parse_class(a), Some(Cmd { name: "mail:send".into(), description: "Send the mail".into(), line: 1 }));
        let b = "#[AsCommand(name: 'app:sync', description: 'x')]\nclass B {}";
        assert_eq!(parse_class(b).unwrap().name, "app:sync");
        let c = "class C {\n protected $name = \"db:prune\";\n}";
        assert_eq!(parse_class(c).unwrap().name, "db:prune");
        assert_eq!(parse_class("class D {}"), None);
        assert_eq!(parse_class("// $signature = 'x:y'\nclass D {}"), None);
        assert_eq!(parse_class("$signature = ''"), None);
        assert_eq!(parse_class("$signature = 'é:日 {a}'").unwrap().name, "é:日");
    }

    #[test]
    fn closures() {
        let t = "<?php\nArtisan::command('inspire', function () {})->purpose('x');\nArtisan::command(\"report:daily {--force}\", fn() => 1);\nArtisan::command(";
        let c = parse_closures(t);
        assert_eq!(c.iter().map(|c| (c.name.as_str(), c.line)).collect::<Vec<_>>(), [("inspire", 1), ("report:daily", 2)]);
    }

    #[test]
    fn discovers_sorted_with_both_sources() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Console/Commands/Sub/Send.php", "class Send {\n protected $signature = 'mail:send';\n protected $description = 'Send it';\n}");
        put(r, "app/Console/Commands/Plain.php", "class Plain {}");
        put(r, "routes/console.php", "<?php\nArtisan::command('inspire', fn() => 1);\n");
        let got = discover(r);
        let labels: Vec<_> = got.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(got.len(), 2);
        assert!(labels[0].starts_with("inspire  routes/console.php"));
        assert!(labels[1].starts_with("mail:send  Send it  app/Console/Commands/Sub/Send.php"));
        assert_eq!(got[1].line, 1);
        assert!(discover(&r.join("none")).is_empty());
    }

    #[test]
    fn bounds_and_symlinks() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        for i in 0..1100 {
            put(r, &format!("app/Console/Commands/C{i}.php"), &format!("$signature = 'c:{i}';"));
        }
        put(r, "app/Console/Commands/Huge.php", &format!("{}\n$signature = 'late:cmd';", "x".repeat(300_000)));
        assert!(discover(r).len() <= MAX_FILES);
        #[cfg(unix)]
        {
            let d2 = tempfile::tempdir().unwrap();
            put(&d2.path().join("real"), "A.php", "$signature = 'a:b';");
            std::fs::create_dir_all(d2.path().join("app/Console")).unwrap();
            std::os::unix::fs::symlink(d2.path().join("real"), d2.path().join("app/Console/Commands")).unwrap();
            assert!(discover(d2.path()).is_empty());
        }
    }

    #[test]
    fn random_text_never_panics() {
        let pieces = ["$signature", " = ", "'", "\"", "AsCommand(", "name:", "$name", "é", "日", "\n", "Artisan::command(", " ", "{a}", ":"];
        let mut s = 11u64;
        for _ in 0..500 {
            let mut t = String::new();
            for _ in 0..60 {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                t.push_str(pieces[(s >> 33) as usize % pieces.len()]);
            }
            let _ = parse_class(&t);
            let _ = parse_closures(&t);
        }
    }
}
