//! Index for the "Go to Migration" palette: Laravel migrations under
//! `database/migrations`, newest first, each with the tables its `up()` touches
//! (`Schema::create|table|dropIfExists|drop|rename('name'...)`).
//!
//! Static text scan, nothing is executed. Bounded: depth 2, 3000 files, 1 MB
//! per file, 5000 entries, symlinks skipped.
use std::path::{Path, PathBuf};

use crate::route_index::RouteTarget;

const MAX_FILE: u64 = 1 << 20;
const MAX_FILES: usize = 3000;
const MAX_DEPTH: usize = 2;
const MAX_ENTRIES: usize = 5000;
const VERBS: &[&str] = &["create", "table", "dropIfExists", "drop", "rename", "dropColumns"];

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_FILES {
        return;
    }
    if depth == 0 && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink()) {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.filter_map(Result::ok) {
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
        if out.len() >= MAX_FILES {
            return;
        }
    }
}

/// `(verb, table)` for each `Schema::verb('table'` in the text, in order,
/// without repeats, plus the 0-based line of the first one.
pub fn parse(text: &str) -> (Vec<(String, String)>, Option<usize>) {
    let mut found: Vec<(String, String)> = Vec::new();
    let mut first = None;
    for (i, line) in text.lines().enumerate() {
        let mut rest = line;
        while let Some(p) = rest.find("Schema::") {
            rest = &rest[p + 8..];
            let Some(verb) = VERBS.iter().find(|v| {
                rest.strip_prefix(**v).is_some_and(|r| r.trim_start().starts_with('('))
            }) else {
                continue;
            };
            let args = rest[verb.len()..].trim_start()[1..].trim_start();
            let Some(q) = args.chars().next().filter(|c| *c == '\'' || *c == '"') else { continue };
            let body = &args[1..];
            let Some(end) = body.find(q) else { continue };
            let table = &body[..end];
            if table.is_empty() || table.len() > 100 || !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
                continue;
            }
            let item = (verb.to_string(), table.to_string());
            if !found.contains(&item) && found.len() < 8 {
                found.push(item);
                first.get_or_insert(i);
            }
        }
    }
    (found, first)
}

pub fn discover(root: &Path) -> Vec<RouteTarget> {
    let mut files = Vec::new();
    walk(&root.join("database/migrations"), 0, &mut files);
    // Newest first: names start with a timestamp, so reverse name order.
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    let mut out = Vec::new();
    for f in files {
        let Ok(md) = std::fs::symlink_metadata(&f) else { continue };
        if !md.is_file() || md.len() > MAX_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let (tables, line) = parse(&text);
        let name = f.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let what = tables
            .iter()
            .map(|(v, t)| format!("{v} {t}"))
            .collect::<Vec<_>>()
            .join(", ");
        let label = if what.is_empty() { format!("migration  {name}") } else { format!("migration  {name}  ({what})") };
        out.push(RouteTarget { label, path: f, line: line.unwrap_or(0) });
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: &str = "<?php\nreturn new class extends Migration {\n    public function up(): void\n    {\n        Schema::create('users', function (Blueprint $t) {});\n        Schema::table(\"posts\", fn ($t) => 1);\n        Schema::create('users', fn () => 1);\n    }\n    public function down(): void\n    {\n        Schema::dropIfExists('users');\n    }\n};\n";

    #[test]
    fn parses_tables() {
        let (t, line) = parse(M);
        assert_eq!(
            t,
            vec![
                ("create".to_string(), "users".to_string()),
                ("table".to_string(), "posts".to_string()),
                ("dropIfExists".to_string(), "users".to_string()),
            ]
        );
        assert_eq!(line, Some(4));
        assert_eq!(parse("Schema::create($x)").0, vec![]);
        assert_eq!(parse("Schema::createX('a')").0, vec![]);
        assert_eq!(parse("Schema::create('a b')").0, vec![]);
        assert_eq!(parse("Schema::").0, vec![]);
    }

    #[test]
    fn discover_newest_first() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let dir = r.join("database/migrations");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("2023_01_01_000000_create_users_table.php"), M).unwrap();
        std::fs::write(dir.join("2024_05_05_000000_add_flag.php"), "<?php\nSchema::table('users', fn () => 1);\n").unwrap();
        std::fs::write(dir.join("2022_01_01_000000_empty.php"), "<?php\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let got = discover(r);
        let labels: Vec<_> = got.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels[0], "migration  2024_05_05_000000_add_flag  (table users)");
        assert_eq!(labels[1], "migration  2023_01_01_000000_create_users_table  (create users, table posts, dropIfExists users)");
        assert_eq!(labels[2], "migration  2022_01_01_000000_empty");
        assert_eq!(labels.len(), 3);
        assert!(discover(&r.join("nope")).is_empty());
    }

    #[test]
    fn never_panics_and_is_fast() {
        for t in ["é", "Schema::create('é", "Schema::create(", "Schema::create('", "Schema::drop(\"é\"", "日Schema::table('a')"] {
            let _ = parse(t);
        }
        let start = std::time::Instant::now();
        let _ = parse(&"Schema::create('a');".repeat(200_000));
        let _ = parse(&"Schema::".repeat(300_000));
        assert!(start.elapsed().as_secs() < 5);
    }
}
