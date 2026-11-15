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

/// Only the first argument of a recognized table call, or the value of an
/// exact `$table =` assignment. Other strings on the line are unrelated.
fn table_prefix(before: &str) -> bool {
    let before = before.trim_end();
    if let Some(lhs) = before.strip_suffix('=') {
        let lhs = lhs.trim_end();
        if let Some(rest) = lhs.strip_suffix("$table") {
            return rest.chars().next_back().is_none_or(|c| {
                !(c.is_ascii_alphanumeric() || matches!(c, '_' | '$'))
            });
        }
    }
    let Some(call) = before.strip_suffix('(') else { return false };
    let call = call.trim_end();
    RECEIVERS.iter().any(|receiver| {
        let receiver = receiver.trim_end_matches('(');
        call.strip_suffix(receiver).is_some_and(|prefix| {
            receiver.starts_with("->") || prefix.chars().next_back().is_none_or(|c| {
                !(c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | ':' | '>'))
            })
        })
    })
}

/// The quoted table name covering byte `col`, including its opening quote.
fn table_at(line: &str, col: usize) -> Option<String> {
    if line.len() > MAX_LINE {
        return None;
    }
    let col = col.min(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !matches!(bytes[i], b'\'' | b'"') {
            i += 1;
            continue;
        }
        let quote = bytes[i];
        let start = i + 1;
        let mut end = start;
        while end < bytes.len() && bytes[end] != quote {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        if end >= bytes.len() {
            return None;
        }
        if col + 1 >= start && col <= end {
            let name = &line[start..end];
            return (!name.is_empty() && name.len() <= 100
                && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.'))
                && table_prefix(&line[..i])).then(|| name.to_string());
        }
        i = end + 1;
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
    // Blueprint commands operate on indexes, constraints or table metadata.
    // Their quoted arguments are not new column declarations. Keep this after
    // reading the first method so `$table->string('email')->unique()` remains
    // a column, while a standalone `$table->unique(...)` does not add one.
    if matches!(method,
        "primary" | "unique" | "index" | "fullText" | "spatialIndex" | "vectorIndex"
        | "rawIndex" | "foreign" | "dropPrimary" | "dropUnique" | "dropIndex"
        | "dropFullText" | "dropSpatialIndex" | "dropVectorIndex" | "dropForeign"
        | "renameIndex" | "rename" | "comment" | "engine" | "charset" | "collation"
    ) {
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

/// Keep only forward method bodies, preserving line/byte positions. Older
/// migration snippets without methods retain their top-level behavior.
fn forward_source(text: &str) -> Option<String> {
    let methods = lapce_index::symbols::php_method_bodies(text).ok()?;
    if methods.is_empty() {
        return Some(text.to_owned());
    }
    let mut source = text.as_bytes().to_vec();
    for byte in &mut source {
        if !matches!(*byte, b'\n' | b'\r') { *byte = b' '; }
    }
    for (name, range) in methods {
        if name.eq_ignore_ascii_case("up") {
            source[range.clone()].copy_from_slice(&text.as_bytes()[range]);
        }
    }
    String::from_utf8(source).ok()
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
        let Some(text) = forward_source(&text) else { continue };
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
    fn table_reference_requires_its_own_argument_or_property_assignment() {
        for line in [
            "DB::table('users')->where('status', 'active')",
            "DB::table('users'); foo('orders')",
            "DB::table('users', 'alias')",
            "protected $table = 'users'; $label = 'orders';",
            "NotDB::table('orders')",
        ] {
            let name = if line.contains("orders") { "orders" } else if line.contains("active") { "active" } else { "alias" };
            assert_eq!(table_at(line, at(line, name)), None, "{line}");
        }
        for line in ["DB::table('users')", "DB::table ( 'users' )", "protected $table = 'users';", "$query->join('users', 'users.id', '=', 'id')"] {
            let col = line.find("'users'").unwrap();
            assert_eq!(table_at(line, col), Some("users".into()), "{line}");
            assert_eq!(table_at(line, col + 2), Some("users".into()), "{line}");
        }
    }

    #[test]
    fn incomplete_migrations_do_not_fall_back_to_rollback_text() {
        assert!(forward_source("<?php return new class { public function down() { Schema::drop('users');").is_none());
    }

    #[test]
    fn forward_source_uses_syntax_not_fake_method_text() {
        let source = r#"<?php
return new class {
    public function down(): void { Schema::drop('users'); }
    public function up(): void {
        $note = 'function down() { }';
        // function down() { }
        Schema::drop('old_users');
    }
};"#;
        let result = forward_source(source).unwrap();
        assert_eq!(result.len(), source.len());
        assert!(!result.contains("Schema::drop('users')"));
        assert!(result.contains("Schema::drop('old_users')"));
        assert_eq!(result.lines().count(), source.lines().count());
    }

    #[test]
    fn rollback_methods_do_not_drop_or_revert_forward_schema() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        put(root, "database/migrations/2026_01_01_create_users.php", r#"<?php
return new class extends Migration {
    public function up(): void {
        Schema::create('users', function ($table) {
            $table->id();
            $table->string('email');
        });
    }
    public function down(): void {
        Schema::dropIfExists('users');
    }
};
"#);
        put(root, "database/migrations/2026_01_02_add_bio.php", r#"<?php
return new class extends Migration {
    public function down(): void {
        Schema::table('users', function ($table) {
            $table->dropColumn('bio');
        });
    }
    public function up(): void {
        Schema::table('users', function ($table) {
            $table->text('bio');
        });
    }
};
"#);
        let line = "DB::table('users')";
        let md = help(Some(root), line, at(line, "users")).unwrap();
        assert!(!md.contains("table dropped"), "{md}");
        assert!(md.contains("`email` string"), "{md}");
        assert!(md.contains("`bio` text"), "{md}");
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
    fn blueprint_commands_are_not_column_declarations() {
        for method in [
            "primary", "unique", "index", "fullText", "spatialIndex", "vectorIndex",
            "rawIndex", "foreign", "dropPrimary", "dropUnique", "dropIndex",
            "dropFullText", "dropSpatialIndex", "dropVectorIndex", "dropForeign",
            "renameIndex", "rename", "comment", "engine", "charset", "collation",
        ] {
            let line = format!("$table->{method}('not_a_column', 'other_name');");
            assert!(column_ops(&line).is_empty(), "{line}");
        }
        assert_eq!(column_ops("$table->string('email')->unique();"),
            vec![("email".into(), "string".into())]);
        assert_eq!(column_ops("$table->foreignId('user_id')->constrained();"),
            vec![("user_id".into(), "foreignId".into())]);
    }

    #[test]
    fn index_drops_and_table_metadata_do_not_pollute_hover_columns() {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "database/migrations/2026_create.php", r#"<?php
Schema::create('users', function ($table) {
    $table->id();
    $table->string('email')->unique();
    $table->dropIndex('users_email_index');
    $table->comment('member records');
    $table->engine('InnoDB');
    $table->foreign('manager_id')->references('id')->on('users');
});
"#);
        let line = "DB::table('users')";
        let md = help(Some(d.path()), line, at(line, "users")).unwrap();
        assert!(md.contains("2 columns from migrations"), "{md}");
        assert!(md.contains("`id`, `email` string"), "{md}");
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
