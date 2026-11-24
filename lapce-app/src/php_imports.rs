//! "Import Class" for PHP: find the fully qualified name of the class under the
//! cursor and insert a sorted `use` line.
//!
//! Candidates come from the PSR-4 maps of `composer.json` (project code) and
//! of `vendor/composer/installed.json` (installed packages). Nothing is run:
//! the directories are walked for a file named `<Class>.php`. Walks are bounded
//! (file count, depth, time), never follow symlinks and skip `node_modules`
//! and dot directories. PHP's own global classes (`Exception`, `DateTime`)
//! have no file to find and are not offered.
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_FILES: usize = 40_000;
const MAX_DEPTH: usize = 14;
const MAX_CANDIDATES: usize = 50;
const MAX_INSTALLED_BYTES: u64 = 16 * 1024 * 1024;
const BUDGET: Duration = Duration::from_millis(1500);

#[derive(Debug, PartialEq, Eq)]
pub enum ImportPlan {
    /// The cursor is not on a usable class name.
    NoName,
    /// Already imported (or declared in the same namespace).
    AlreadyAvailable(String),
    /// No `<?php` header to anchor the edit.
    NoPlace,
    /// Nothing found.
    NotFound,
    /// One candidate: byte offset and text to insert.
    Insert { fqcn: String, offset: usize, text: String },
    /// Several candidates to pick from.
    Choose { short: String, candidates: Vec<String> },
}

/// The class name (`Request`, or the head of `Str::of`) at byte `col` of `line`.
pub fn name_at(line: &str, col: usize) -> Option<String> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let ok = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let start = line[..col]
        .char_indices()
        .rev()
        .take_while(|(_, c)| ok(*c))
        .last()
        .map(|(i, _)| i)
        .unwrap_or(col);
    let end = col + line[col..].find(|c: char| !ok(c)).unwrap_or(line.len() - col);
    let w = &line[start..end];
    let first = w.chars().next()?;
    (first.is_ascii_uppercase()).then(|| w.to_string())
}

struct Header {
    namespace: Option<String>,
    /// (fqcn, short name) of plain class imports.
    uses: Vec<(String, String, usize)>,
    /// Byte offset after the namespace (or open tag) line.
    anchor: Option<usize>,
    crlf: bool,
}

fn parse_header(text: &str) -> Header {
    let crlf = text.contains("\r\n");
    let mut h = Header { namespace: None, uses: vec![], anchor: None, crlf };
    let mut pos = 0;
    let mut open_anchor = None;
    for raw in text.split_inclusive('\n') {
        let start = pos;
        pos += raw.len();
        let line = raw.trim();
        if open_anchor.is_none() && line.starts_with("<?php") {
            open_anchor = Some(pos);
            continue;
        }
        if let Some(rest) = line.strip_prefix("namespace ") {
            if let Some(ns) = rest.strip_suffix(';') {
                h.namespace = Some(ns.trim().trim_start_matches('\\').to_string());
                h.anchor = Some(pos);
                continue;
            }
        }
        if let Some(rest) = line.strip_prefix("use ") {
            if rest.starts_with("function ") || rest.starts_with("const ") {
                continue;
            }
            let Some(body) = rest.strip_suffix(';') else { continue };
            if let Some((prefix, group)) = body.split_once('{') {
                let prefix = prefix.trim().trim_start_matches('\\');
                for item in group.trim_end_matches('}').split(',') {
                    let item = item.trim();
                    if item.is_empty() {
                        continue;
                    }
                    let full = format!("{prefix}{item}");
                    push_use(&mut h, &full, start);
                }
            } else {
                push_use(&mut h, body.trim().trim_start_matches('\\'), start);
            }
            continue;
        }
        let decl = ["class ", "abstract ", "final ", "interface ", "trait ", "enum ", "readonly ", "function "];
        if decl.iter().any(|d| line.starts_with(d)) {
            break;
        }
    }
    if h.anchor.is_none() {
        h.anchor = open_anchor;
    }
    h
}

fn push_use(h: &mut Header, spec: &str, line_start: usize) {
    let (full, alias) = match spec.split_once(" as ") {
        Some((f, a)) => (f.trim(), Some(a.trim())),
        None => (spec, None),
    };
    let short = alias.unwrap_or_else(|| full.rsplit('\\').next().unwrap_or(full));
    h.uses.push((full.to_string(), short.to_string(), line_start));
}


/// Navigate only code in the current PHP lexer snapshot, never multiline
/// strings, heredocs, comments or the HTML region of a mixed template.
pub fn definition_at(root: &Path, source: &str, offset: usize) -> Option<(PathBuf, usize)> {
    if source.len() > 1024 * 1024 || offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let snapshot = lapce_core::lexilla::highlight(
        lapce_core::language::LapceLanguage::Php, &lapce_xi_rope::Rope::from(source),
    )?;
    let style = *snapshot.styles().get(offset)?;
    if matches!(snapshot.scope(style), Some("comment" | "string")) { return None; }
    let start = source[..offset].rfind('\n').map_or(0, |i| i + 1);
    let end = source[offset..].find('\n').map_or(source.len(), |i| offset + i);
    definition(root, source, &source[start..end], offset - start)
}

/// Resolve an explicit class reference without executing PHP or guessing a basename.
/// Dynamic variables, methods, comments and quoted strings are left to the LSP.
fn definition(root: &Path, text: &str, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    if text.len() > 1024 * 1024 || col > line.len() || !line.is_char_boundary(col) {
        return None;
    }
    let valid = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '\\';
    let start = line[..col].char_indices().rev().find(|(_, c)| !valid(*c)).map_or(0, |(i,c)| i+c.len_utf8());
    let end = col + line[col..].find(|c: char| !valid(c)).unwrap_or(line.len()-col);
    let token = &line[start..end];
    if token.is_empty() || token.len() > 300 || token.starts_with(|c: char| !c.is_ascii_uppercase() && c != '\\') {
        return None;
    }
    let before = &line[..start];
    // Conservative line-level exclusion. Do not turn text in a string/comment
    // into a competing local definition result.
    if before.contains(['\'', '"']) || before.contains("//") || before.contains("/*")
        || before.contains('#') || before.ends_with('$') || before.ends_with("->")
        || before.ends_with("::") || before.trim_start().starts_with('*') {
        return None;
    }
    let after = line[end..].trim_start();
    let class_context = after.starts_with("::") || before.trim_end().ends_with("new")
        || before.trim_start().starts_with("use ") || before.trim_end().ends_with("extends")
        || before.trim_end().ends_with("implements") || after.starts_with('$');
    if !class_context { return None; }
    let h = parse_header(text);
    let head = token.trim_start_matches('\\').split('\\').next()?;
    let tail = &token.trim_start_matches('\\')[head.len()..];
    let imported: Vec<_> = h.uses.iter().filter(|(_, short, _)| short.eq_ignore_ascii_case(head)).collect();
    if imported.len() > 1 { return None; }
    let fqcn = if token.starts_with('\\') {
        token.trim_start_matches('\\').to_string()
    } else if let Some((full, _, _)) = imported.first() {
        format!("{full}{tail}")
    } else if let Some(ns) = h.namespace {
        format!("{ns}\\{token}")
    } else { token.to_string() };
    let mut maps = crate::class_refs::psr4(root);
    maps.extend(vendor_psr4(root));
    if !root.join("composer.json").exists() {
        maps.push(("App\\".to_string(), root.join("app")));
    }
    let mut found: Vec<(PathBuf, usize)> = Vec::new();
    for (prefix, directory) in maps {
        let Some(rest) = fqcn.strip_prefix(prefix.as_str()) else { continue; };
        if rest.is_empty() || rest.split('\\').any(|s| s.is_empty() || s == "..") { continue; }
        let path = directory.join(format!("{}.php", rest.replace('\\', "/")));
        let Ok(meta) = std::fs::metadata(&path) else { continue; };
        if !meta.is_file() || meta.len() > 1024 * 1024 { continue; }
        let Ok(body) = std::fs::read_to_string(&path) else { continue; };
        let Some((decl, line)) = crate::app_class_index::class_decl(&body) else { continue; };
        if !decl.eq_ignore_ascii_case(fqcn.rsplit('\\').next()?) { continue; }
        if !found.iter().any(|(p, _)| p == &path) { found.push((path, line)); }
    }
    if found.len() == 1 { found.pop() } else { None }
}

/// Walk `dir` for `<short>.php`, pushing the PSR-4 name built from `prefix`.
fn walk(
    dir: &Path,
    prefix: &str,
    rel: &mut Vec<String>,
    short: &str,
    out: &mut Vec<String>,
    seen: &mut usize,
    deadline: Instant,
) {
    if rel.len() > MAX_DEPTH || *seen > MAX_FILES || Instant::now() > deadline {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let want = format!("{short}.php");
    let mut dirs = vec![];
    for e in rd.flatten() {
        *seen += 1;
        if *seen > MAX_FILES {
            return;
        }
        let Ok(ft) = e.file_type() else { continue };
        let name = e.file_name().to_string_lossy().to_string();
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            if !name.starts_with('.') && name != "node_modules" {
                dirs.push(name);
            }
        } else if ft.is_file() && name == want {
            let mut parts = rel.clone();
            parts.push(short.to_string());
            out.push(format!("{prefix}{}", parts.join("\\")));
        }
    }
    dirs.sort();
    for d in dirs {
        rel.push(d.clone());
        walk(&dir.join(&d), prefix, rel, short, out, seen, deadline);
        rel.pop();
    }
}

fn vendor_psr4(root: &Path) -> Vec<(String, PathBuf)> {
    let base = root.join("vendor/composer");
    let file = base.join("installed.json");
    let ok = std::fs::metadata(&file)
        .map(|m| m.is_file() && m.len() <= MAX_INSTALLED_BYTES)
        .unwrap_or(false);
    if !ok {
        return vec![];
    }
    let Some(doc) = std::fs::read_to_string(&file)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    else {
        return vec![];
    };
    let pkgs = doc.get("packages").and_then(|p| p.as_array()).or_else(|| doc.as_array());
    let Some(pkgs) = pkgs else { return vec![] };
    let Ok(vendor) = root.join("vendor").canonicalize() else { return vec![] };
    let mut out = vec![];
    for p in pkgs {
        let Some(ip) = p.get("install-path").and_then(|v| v.as_str()) else { continue };
        let Ok(pkg_dir) = base.join(ip).canonicalize() else { continue };
        if !pkg_dir.starts_with(&vendor) {
            continue;
        }
        let Some(map) = p.get("autoload").and_then(|a| a.get("psr-4")).and_then(|m| m.as_object()) else {
            continue;
        };
        for (prefix, dirs) in map {
            let list: Vec<&str> = match dirs {
                serde_json::Value::String(s) => vec![s.as_str()],
                serde_json::Value::Array(a) => a.iter().filter_map(|v| v.as_str()).collect(),
                _ => vec![],
            };
            for d in list {
                if d.split('/').any(|s| s == "..") {
                    continue;
                }
                out.push((prefix.clone(), pkg_dir.join(d.trim_start_matches("./"))));
            }
        }
    }
    out
}

/// Fully qualified names of classes called `short`, project code first.
pub fn candidates(root: &Path, short: &str) -> Vec<String> {
    let deadline = Instant::now() + BUDGET;
    let mut seen = 0usize;
    let mut out: Vec<String> = vec![];
    let mut dirs = crate::class_refs::psr4(root);
    dirs.extend(vendor_psr4(root));
    let mut walked: Vec<PathBuf> = vec![];
    for (prefix, dir) in dirs {
        if out.len() >= MAX_CANDIDATES || Instant::now() > deadline {
            break;
        }
        if walked.contains(&dir) || !dir.is_dir() {
            continue;
        }
        walked.push(dir.clone());
        let mut found = vec![];
        walk(&dir, &prefix, &mut vec![], short, &mut found, &mut seen, deadline);
        for f in found {
            if !out.contains(&f) {
                out.push(f);
            }
        }
    }
    out.truncate(MAX_CANDIDATES);
    out
}

/// Where and what to insert so that `use <fqcn>;` lands in sorted position.
pub fn insertion(text: &str, fqcn: &str) -> Option<(usize, String)> {
    let h = parse_header(text);
    let nl = if h.crlf { "\r\n" } else { "\n" };
    let line = format!("use {fqcn};{nl}");
    let key = fqcn.to_lowercase();
    if h.uses.is_empty() {
        let anchor = h.anchor?;
        return Some((anchor, format!("{nl}{line}")));
    }
    for (full, _, start) in &h.uses {
        if full.to_lowercase() > key {
            return Some((*start, line));
        }
    }
    // After the last import line.
    let (_, _, last) = h.uses.last()?;
    let end = text[*last..].find('\n').map(|i| last + i + 1).unwrap_or(text.len());
    if end == text.len() && !text.ends_with('\n') {
        return Some((end, format!("{nl}{line}").trim_end().to_string()));
    }
    Some((end, line))
}

/// Decide what "Import Class" should do for `short` in the file `text`.
pub fn plan(root: &Path, text: &str, short: &str) -> ImportPlan {
    if short.is_empty() {
        return ImportPlan::NoName;
    }
    let h = parse_header(text);
    for (full, s, _) in &h.uses {
        if s == short {
            return ImportPlan::AlreadyAvailable(full.clone());
        }
    }
    let mut cands = candidates(root, short);
    if let Some(ns) = &h.namespace {
        let same = format!("{ns}\\{short}");
        if cands.contains(&same) {
            return ImportPlan::AlreadyAvailable(same);
        }
    }
    if cands.is_empty() {
        return ImportPlan::NotFound;
    }
    if h.anchor.is_none() {
        return ImportPlan::NoPlace;
    }
    if cands.len() == 1 {
        let fqcn = cands.remove(0);
        return match insertion(text, &fqcn) {
            Some((offset, t)) => ImportPlan::Insert { fqcn, offset, text: t },
            None => ImportPlan::NoPlace,
        };
    }
    ImportPlan::Choose { short: short.to_string(), candidates: cands }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn project() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        fs::write(r.join("composer.json"), r#"{"autoload":{"psr-4":{"App\\":"app/"}}}"#).unwrap();
        fs::create_dir_all(r.join("app/Models")).unwrap();
        fs::create_dir_all(r.join("app/Http/Controllers")).unwrap();
        fs::write(r.join("app/Models/User.php"), "<?php").unwrap();
        fs::write(r.join("app/Http/Controllers/Controller.php"), "<?php").unwrap();
        d
    }

    const SRC: &str = "<?php\n\nnamespace App\\Http\\Controllers;\n\nuse App\\Models\\Post;\nuse Illuminate\\Http\\Request;\n\nclass HomeController\n{\n    use Foo;\n}\n";

    #[test]
    fn name_under_cursor() {
        assert_eq!(name_at("$u = new User();", 10).as_deref(), Some("User"));
        assert_eq!(name_at("$u = new User();", 13).as_deref(), Some("User"));
        assert_eq!(name_at("$user = 1;", 3), None);
        assert_eq!(name_at("é", 1).as_deref(), None);
        assert_eq!(name_at("Str::of()", 1).as_deref(), Some("Str"));
        assert_eq!(name_at("", 5), None);
    }

    #[test]
    fn inserts_sorted_between_imports() {
        let (off, t) = insertion(SRC, "App\\Models\\User").unwrap();
        let mut out = SRC.to_string();
        out.insert_str(off, &t);
        assert!(out.contains("use App\\Models\\Post;\nuse App\\Models\\User;\nuse Illuminate"), "{out}");
    }

    #[test]
    fn inserts_after_last_import() {
        let (off, t) = insertion(SRC, "Zed\\Thing").unwrap();
        let mut out = SRC.to_string();
        out.insert_str(off, &t);
        assert!(out.contains("Request;\nuse Zed\\Thing;\n\nclass"), "{out}");
    }

    #[test]
    fn first_import_goes_after_namespace() {
        let src = "<?php\n\nnamespace App;\n\nclass A {}\n";
        let (off, t) = insertion(src, "X\\Y").unwrap();
        let mut out = src.to_string();
        out.insert_str(off, &t);
        assert_eq!(out, "<?php\n\nnamespace App;\n\nuse X\\Y;\n\nclass A {}\n");
    }

    #[test]
    fn first_import_without_namespace() {
        let src = "<?php\n\nclass A {}\n";
        let (off, t) = insertion(src, "X\\Y").unwrap();
        let mut out = src.to_string();
        out.insert_str(off, &t);
        assert_eq!(out, "<?php\n\nuse X\\Y;\n\nclass A {}\n");
    }

    #[test]
    fn crlf_is_kept() {
        let src = "<?php\r\n\r\nnamespace App;\r\n\r\nclass A {}\r\n";
        let (_, t) = insertion(src, "X\\Y").unwrap();
        assert_eq!(t, "\r\nuse X\\Y;\r\n");
    }

    #[test]
    fn trait_use_in_class_is_not_an_import() {
        let h = parse_header(SRC);
        assert_eq!(h.uses.len(), 2);
    }

    #[test]
    fn group_alias_and_function_imports() {
        let src = "<?php\nnamespace A;\nuse B\\{C, D as E};\nuse function F\\g;\nuse H\\I as J;\nclass X {}\n";
        let h = parse_header(src);
        let shorts: Vec<_> = h.uses.iter().map(|u| u.1.as_str()).collect();
        assert_eq!(shorts, ["C", "E", "J"]);
    }

    #[test]
    fn plan_single_candidate_inserts() {
        let d = project();
        match plan(d.path(), SRC, "User") {
            ImportPlan::Insert { fqcn, .. } => assert_eq!(fqcn, "App\\Models\\User"),
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn plan_already_imported_same_namespace_and_missing() {
        let d = project();
        assert!(matches!(plan(d.path(), SRC, "Post"), ImportPlan::AlreadyAvailable(_)));
        assert!(matches!(plan(d.path(), SRC, "Controller"), ImportPlan::AlreadyAvailable(_)));
        assert_eq!(plan(d.path(), SRC, "Nope"), ImportPlan::NotFound);
        assert_eq!(plan(d.path(), SRC, ""), ImportPlan::NoName);
    }

    #[test]
    fn plan_multiple_candidates_choose() {
        let d = project();
        fs::create_dir_all(d.path().join("app/Admin")).unwrap();
        fs::write(d.path().join("app/Admin/User.php"), "<?php").unwrap();
        match plan(d.path(), SRC, "User") {
            ImportPlan::Choose { candidates, .. } => {
                assert_eq!(candidates.len(), 2);
                assert!(candidates.contains(&"App\\Admin\\User".to_string()));
            }
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn vendor_packages_are_searched() {
        let d = project();
        let r = d.path();
        fs::create_dir_all(r.join("vendor/composer")).unwrap();
        fs::create_dir_all(r.join("vendor/laravel/framework/src/Illuminate/Support")).unwrap();
        fs::write(r.join("vendor/laravel/framework/src/Illuminate/Support/Str.php"), "<?php").unwrap();
        fs::write(
            r.join("vendor/composer/installed.json"),
            r#"{"packages":[{"name":"laravel/framework","install-path":"../laravel/framework","autoload":{"psr-4":{"Illuminate\\":"src/Illuminate/"}}}]}"#,
        )
        .unwrap();
        assert_eq!(candidates(r, "Str"), vec!["Illuminate\\Support\\Str".to_string()]);
    }

    #[test]
    fn vendor_path_escape_is_ignored() {
        let d = project();
        let r = d.path();
        fs::create_dir_all(r.join("vendor/composer")).unwrap();
        fs::create_dir_all(r.join("outside")).unwrap();
        fs::write(r.join("outside/Evil.php"), "<?php").unwrap();
        fs::write(
            r.join("vendor/composer/installed.json"),
            r#"[{"install-path":"../../outside","autoload":{"psr-4":{"Evil\\":""}}}]"#,
        )
        .unwrap();
        assert!(candidates(r, "Evil").is_empty());
    }

    #[test]
    fn garbage_inputs_do_not_panic() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(plan(d.path(), "", "A"), ImportPlan::NotFound);
        fs::write(d.path().join("composer.json"), "{{{").unwrap();
        assert!(candidates(d.path(), "A").is_empty());
        assert!(insertion("", "A\\B").is_none());
    }
}


#[cfg(test)]
mod definition_tests {
    use super::*;
    fn fixture() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for (file, body) in [
            ("composer.json", r#"{"autoload":{"psr-4":{"App\\":"app/"}}}"#),
            ("app/Models/Post.php", "<?php\nnamespace App\\Models;\nclass Post {}\n"),
            ("app/Services/ReceiptService.php", "<?php\nnamespace App\\Services;\nclass ReceiptService {}\n"),
        ] {
            let p = d.path().join(file);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        d
    }
    #[test]
    fn definition_imported_model_and_alias_new_class() {
        let d = fixture();
        let text = "<?php\nuse App\\Models\\Post;\nPost::query();\n";
        assert_eq!(definition(d.path(), text, "Post::query();", 2), Some((d.path().join("app/Models/Post.php"), 2)));
        let text = "<?php\nuse App\\Services\\ReceiptService as Receipt;\nnew Receipt();\n";
        assert_eq!(definition(d.path(), text, "new Receipt();", 6), Some((d.path().join("app/Services/ReceiptService.php"), 2)));
    }
    #[test]
    fn definition_namespace_qualified_and_group_imports() {
        let d = fixture();
        for (text, line, at) in [
            ("<?php\nnamespace App\\Models;\nPost::query();", "Post::query();", 2),
            ("<?php", "new \\App\\Services\\ReceiptService();", 12),
            ("<?php\nuse App\\Models\\{Post};", "Post::query();", 2),
        ] {
            assert!(definition(d.path(), text, line, at).is_some(), "{line}");
        }
    }
    #[test]
    fn definition_at_excludes_multiline_noncode() {
        let d = fixture();
        for body in ["/*\nPost::query();\n*/", "$x = 'first\nPost::query();\n';", "$x = <<<TXT\nPost::query();\nTXT;"] {
            let text = format!("<?php\nuse App\\Models\\Post;\n{body}");
            let offset = text.rfind("Post::").unwrap() + 2;
            assert_eq!(definition_at(d.path(), &text, offset), None, "{body}");
        }
        let text = "<?php\nuse App\\Models\\Post;\nPost::query();";
        assert!(definition_at(d.path(), text, text.rfind("Post::").unwrap()+2).is_some());
    }
    #[test]
    fn definition_does_not_guess_dynamic_methods_strings_or_missing_imports() {
        let d = fixture();
        let text = "<?php\nuse App\\Models\\Post;";
        for (line, at) in [("$Post::query();", 3), ("$x->Post();", 6), ("// Post::query();", 5), ("'Post::query()'", 3), ("Post::query();", 8)] {
            assert_eq!(definition(d.path(), text, line, at), None, "{line}");
        }
        assert_eq!(definition(d.path(), "<?php", "Post::query();", 2), None);
        let text = "<?php\nuse App\\Models\\Post;\nuse Other\\Post;";
        assert_eq!(definition(d.path(), text, "Post::query();", 2), None);
    }
}
