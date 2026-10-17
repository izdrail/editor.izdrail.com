//! Hover a database table name - `DB::table('users')`, `Schema::table(...)`,
//! `->from('users')`, `->join(...)`, or a model's `protected $table = 'users'` -
//! and see the column list rebuilt from the project's migrations. Static text
//! scan, nothing is executed and the live database is never asked.

use std::path::{Path, PathBuf};

const MAX_LINE: usize = 4096;
const MAX_FILE: u64 = 512 * 1024;
const MAX_FILES: usize = 300;
const MAX_DEPTH: usize = 2;
/// Lines scanned for `$table->...` calls after a `Schema::create/table`.
const BODY: usize = 120;
const MAX_COLUMNS: usize = 64;

/// Receivers whose first quoted argument is a table name.
const RECEIVERS: &[&str] = &[
    "DB::table(",
    "->table(",
    "Schema::create(",
    "Schema::table(",
    "Schema::drop(",
    "Schema::dropIfExists(",
    "Schema::rename(",
    "->from(",
    "->join(",
    "->leftJoin(",
    "->rightJoin(",
    "->crossJoin(",
];

/// The quoted table name covering byte `col`, gated on a receiver (or a
/// model's `$table =` property).
fn table_at(line: &str, col: usize) -> Option<String> {
    if line.len() > MAX_LINE {
        return None;
    }
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    for q in ['\'', '"'] {
        for (i, _) in line.match_indices(q) {
            let start = i + 1;
            let Some(len) = line[start..].find(q) else { break };
            let end = start + len;
            if col >= start && col <= end {
                let s = &line[start..end];
                let ok = !s.is_empty()
                    && s.len() <= 100
                    && s.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
                if !ok {
                    return None;
                }
                let gated = RECEIVERS.iter().any(|r| line.contains(r))
                    || line[..i].contains("$table");
                return gated.then(|| s.to_string());
            }
        }
    }
    None
}

/// `(name, type)` pairs from one `$table->...;` line; drops and renames are
/// returned as `("-name", "")` / `("a>b", "")` markers for the caller.
fn column_ops(l: &str) -> Vec<(String, String)> {
    let Some(p) = l.find("$table->") else { return Vec::new() };
    let rest = &l[p + 8..];
    let Some(open) = rest.find('(') else { return Vec::new() };
    let method = rest[..open].trim();
    if !method.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Vec::new();
    }
    let args = &rest[open + 1..];
    let mut names = Vec::new();
    let mut r = args;
    while let Some(qi) = r.find(['\'', '"']) {
        let q = r.as_bytes()[qi] as char;
        let body = &r[qi + 1..];
        let Some(end) = body.find(q) else { break };
        names.push(body[..end].to_string());
        r = &body[end + 1..];
        if names.len() >= 4 {
            break;
        }
    }
    match method {
        "id" => vec![("id".into(), String::new())],
        "timestamps" | "nullableTimestamps" => {
            vec![("created_at".into(), String::new()), ("updated_at".into(), String::new())]
        }
        "softDeletes" | "softDeletesTz" => vec![("deleted_at".into(), String::new())],
        "rememberToken" => vec![("remember_token".into(), String::new())],
        "dropColumn" | "dropConstrainedForeignId" | "dropMorphs" => {
            names.into_iter().map(|n| (format!("-{n}"), String::new())).collect()
        }
        "renameColumn" if names.len() >= 2 => {
            vec![(format!("{}>{}", names[0], names[1]), String::new())]
        }
        "morphs" | "nullableMorphs" | "uuidMorphs" | "ulidMorphs" if !names.is_empty() => {
            vec![
                (format!("{}_id", names[0]), method.into()),
                (format!("{}_type", names[0]), method.into()),
            ]
        }
        _ => match names.into_iter().next() {
            Some(n) => vec![(n, method.to_string())],
            None => Vec::new(),
        },
    }
}

/// Lines of the closure body following the `Schema::create/table` at `start`.
fn body_lines<'a>(lines: &'a [&'a str], start: usize) -> impl Iterator<Item = &'a str> + 'a {
    lines
        .iter()
        .skip(start + 1)
        .take(BODY)
        .take_while(|l| !l.contains("});") && !l.contains("Schema::"))
        .copied()
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_FILES {
        return;
    }
    if depth == 0
        && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink())
    {
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

/// Rebuild the column list for `table` from migrations, oldest first.
/// Returns `(columns, create-file, dropped-by)` when any migration touches it.
fn columns_for(root: &Path, table: &str) -> Option<(Vec<(String, String)>, PathBuf, Option<PathBuf>)> {
    let mut files = Vec::new();
    walk(&root.join("database/migrations"), 0, &mut files);
    // Oldest first: names start with a timestamp, so name order is time order.
    files.sort_by_key(|f| f.file_name().map(|n| n.to_os_string()));
    let mut columns: Vec<(String, String)> = Vec::new();
    let mut create_file: Option<PathBuf> = None;
    let mut dropped_by: Option<PathBuf> = None;
    let mut touched = false;
    for f in files {
        let Ok(md) = std::fs::symlink_metadata(&f) else { continue };
        if !md.is_file() || md.len() > MAX_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let lines: Vec<&str> = text.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            for verb in ["create", "table"] {
                let needle = format!("Schema::{verb}(");
                let Some(p) = l.find(&needle) else { continue };
                let args = &l[p + needle.len()..];
                let Some(q) = args.chars().next().filter(|c| *c == '\'' || *c == '"')
                else {
                    continue;
                };
                let Some(end) = args[1..].find(q) else { continue };
                if &args[1..1 + end] != table {
                    continue;
                }
                touched = true;
                if verb == "create" {
                    columns.clear();
                    dropped_by = None;
                    create_file = Some(f.clone());
                }
                for op in body_lines(&lines, i).flat_map(column_ops) {
                    if let Some(name) = op.0.strip_prefix('-') {
                        columns.retain(|(n, _)| n != name);
                    } else if let Some((a, b)) = op.0.split_once('>') {
                        if let Some(slot) = columns.iter_mut().find(|(n, _)| n == a) {
                            *slot = (b.to_string(), op.1.clone());
                        }
                    } else if !columns.iter().any(|(n, _)| n == &op.0)
                        && columns.len() < MAX_COLUMNS
                    {
                        columns.push(op);
                    }
                }
            }
            for verb in ["dropIfExists", "drop"] {
                let needle = format!("Schema::{verb}(");
                let Some(p) = l.find(&needle) else { continue };
                let args = &l[p + needle.len()..];
                let Some(q) = args.chars().next().filter(|c| *c == '\'' || *c == '"')
                else {
                    continue;
                };
                let Some(end) = args[1..].find(q) else { continue };
                if &args[1..1 + end] == table {
                    touched = true;
                    dropped_by = Some(f.clone());
                }
            }
        }
    }
    touched.then_some((columns, create_file?, dropped_by))
}

/// Markdown hover for the table name at byte `col` of `line`.
pub fn help(root: Option<&Path>, line: &str, col: usize) -> Option<String> {
    let root = root?;
    let table = table_at(line, col)?;
    let (columns, file, dropped) = columns_for(root, &table)?;
    let rel = file.strip_prefix(root).unwrap_or(&file);
    let mut md = if let Some(d) = dropped {
        let drel = d.strip_prefix(root).unwrap_or(&d);
        format!(
            "**`{table}`** - table dropped by `{}`\n\n_{}_",
            drel.display(),
            rel.display()
        )
    } else {
        let shown: Vec<String> = columns
            .iter()
            .take(24)
            .map(|(n, t)| {
                if t.is_empty() {
                    format!("`{n}`")
                } else {
                    format!("`{n}` {t}")
                }
            })
            .collect();
        let more = if columns.len() > 24 {
            format!(", +{} more", columns.len() - 24)
        } else {
            String::new()
        };
        format!(
            "**`{table}`** - {} column{} from migrations\n\n{}{}\n\n_{}_",
            columns.len(),
            if columns.len() == 1 { "" } else { "s" },
            shown.join(", "),
            more,
            rel.display()
        )
    };
    md.push_str("\n\n_Static read of migrations; the live database is not asked._");
    Some(md)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
    }

    fn at(line: &str, needle: &str) -> usize {
        line.rfind(needle).unwrap() + 1
    }

    #[test]
    fn table_columns_come_from_migrations() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "database/migrations/2014_10_12_000000_create_users_table.php",
            "<?php\nreturn new class {\n    public function up(): void\n    {\n        Schema::create('users', function (Blueprint $table) {\n            $table->id();\n            $table->string('name');\n            $table->string('email')->unique();\n            $table->timestamps();\n        });\n    }\n};\n",
        );
        put(
            r,
            "database/migrations/2024_05_05_000000_add_bio_to_users_table.php",
            "<?php\nreturn new class {\n    public function up(): void\n    {\n        Schema::table('users', function (Blueprint $table) {\n            $table->text('bio')->nullable();\n            $table->dropColumn('email');\n        });\n    }\n};\n",
        );
        let root = Some(r);

        let l = "$users = DB::table('users')->get();";
        let md = help(root, l, at(l, "users")).unwrap();
        assert!(md.contains("**`users`** - 5 columns from migrations"), "{md}");
        assert!(md.contains("`id`, `name` string,"), "{md}");
        assert!(md.contains("`created_at`, `updated_at`, `bio` text"), "{md}");
        assert!(!md.contains("`email`"), "{md}");
        assert!(md.contains("2014_10_12_000000_create_users_table.php"), "{md}");

        // Model property form works; unknown tables and non-receivers: nothing.
        let l = "    protected $table = 'users';";
        assert!(help(root, l, at(l, "users")).is_some());
        let l = "DB::table('ghosts')->count();";
        assert_eq!(help(root, l, at(l, "ghosts")), None);
        let l = "foo('users')";
        assert_eq!(help(root, l, at(l, "users")), None);
        assert_eq!(help(None, "DB::table('users')", 18), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        let root = Some(d.path());
        for l in ["DB::table('", "'", "$table->", "table('é')", "table('日本')", "()"] {
            for c in 0..=l.len() + 2 {
                let _ = help(root, l, c);
            }
        }
    }
}
