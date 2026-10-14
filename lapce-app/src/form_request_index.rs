//! Index for the "Go to Form Request" palette: Laravel form request classes
//! under `app/Http/Requests` and the field names in their `rules()` array.
//!
//! A class counts when it `extends FormRequest` (or any `...Request`). A
//! field is a quoted key at the top level of the array returned by `rules()`.
//! Static text scan, nothing is executed. Bounded: depth 6, 1500 files,
//! 1 MB per file, 100 fields per class, 5000 entries, symlinks skipped.
use std::path::{Path, PathBuf};

use crate::route_index::RouteTarget;

const MAX_FILE: u64 = 1 << 20;
const MAX_FILES: usize = 1500;
const MAX_DEPTH: usize = 6;
const MAX_ENTRIES: usize = 5000;
const MAX_FIELDS: usize = 100;

#[derive(Debug, PartialEq, Eq)]
pub struct Parsed {
    pub class: String,
    pub class_line: usize,
    pub rules_line: Option<usize>,
    /// `(field, 0-based line)`.
    pub fields: Vec<(String, usize)>,
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

fn ident_after<'a>(s: &'a str, kw: &str) -> Option<&'a str> {
    let i = s.find(kw)?;
    let rest = s[i + kw.len()..].trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '\\'))
        .unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

/// `line` with quoted strings blanked, so brackets inside them don't count.
fn strip_strings(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut q: Option<char> = None;
    let mut esc = false;
    for c in line.chars() {
        match q {
            Some(qc) => {
                if esc {
                    esc = false;
                } else if c == '\\' {
                    esc = true;
                } else if c == qc {
                    q = None;
                }
            }
            None => {
                if c == '\'' || c == '"' {
                    q = Some(c);
                } else {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// Quoted key at the start of `line` followed by `=>`.
fn key_of(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let q = t.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let body = &t[1..];
    let end = body.find(q)?;
    body[end + 1..].trim_start().starts_with("=>").then(|| &body[..end])
}

/// Parse one PHP file; `None` when it declares no form request class.
pub fn parse(text: &str) -> Option<Parsed> {
    let mut class: Option<(String, usize)> = None;
    let mut rules_line = None;
    let mut fields = Vec::new();
    let mut in_rules = false;
    let mut depth: i32 = 0;
    let mut started = false;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with("//") || line.starts_with('*') || line.starts_with('#') || line.starts_with("/*") {
            continue;
        }
        if class.is_none() {
            if line.starts_with("class ") || line.starts_with("final class ") || line.starts_with("abstract class ") {
                let name = ident_after(line, "class ")?;
                let parent = ident_after(line, " extends ")?;
                let parent = parent.rsplit('\\').next().unwrap_or(parent);
                if parent.ends_with("Request") {
                    class = Some((name.to_string(), i));
                } else {
                    return None;
                }
            }
            continue;
        }
        if !in_rules {
            if line.contains("function rules") && rules_line.is_none() {
                in_rules = true;
                rules_line = Some(i);
                depth = 0;
                started = false;
            }
            continue;
        }
        // Inside rules(): the returned array opens at depth 1.
        if depth == 1 && fields.len() < MAX_FIELDS {
            if let Some(k) = key_of(line) {
                fields.push((k.to_string(), i));
            }
        }
        for c in strip_strings(line).chars() {
            match c {
                '[' | '(' => depth += 1,
                ']' | ')' => depth -= 1,
                _ => {}
            }
        }
        if depth > 0 {
            started = true;
        }
        if started && depth <= 0 {
            in_rules = false;
        }
        // A rules() with no array after 5000 lines is not worth following.
        if !started && i > rules_line.unwrap_or(0) + 200 {
            in_rules = false;
        }
    }
    class.map(|(class, class_line)| Parsed { class, class_line, rules_line, fields })
}

pub fn discover(root: &Path) -> Vec<RouteTarget> {
    let mut files = Vec::new();
    walk(&root.join("app/Http/Requests"), 0, &mut files);
    let mut out = Vec::new();
    for f in files {
        let Ok(md) = std::fs::symlink_metadata(&f) else { continue };
        if !md.is_file() || md.len() > MAX_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let Some(p) = parse(&text) else { continue };
        let shown = f.strip_prefix(root).unwrap_or(&f).display().to_string();
        out.push(RouteTarget { label: format!("{}  {shown}", p.class), path: f.clone(), line: p.class_line });
        for (field, line) in &p.fields {
            out.push(RouteTarget { label: format!("{}  rule: {field}", p.class), path: f.clone(), line: *line });
        }
        if out.len() >= MAX_ENTRIES {
            out.truncate(MAX_ENTRIES);
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?php
namespace App\Http\Requests;

use Illuminate\Foundation\Http\FormRequest;

class StoreUserRequest extends FormRequest
{
    public function authorize(): bool { return true; }

    public function rules(): array
    {
        return [
            'name' => ['required', 'string', 'regex:/[a-z]/'],
            'email' => 'required|email',
            'address' => [
                'street' => 'x',
            ],
            "tags.*" => 'string',
        ];
    }

    public function messages(): array
    {
        return ['name.required' => 'Name!'];
    }
}
"#;

    #[test]
    fn parses_top_level_fields_only() {
        let p = parse(SAMPLE).unwrap();
        assert_eq!(p.class, "StoreUserRequest");
        assert_eq!(p.class_line, 5);
        assert_eq!(p.rules_line, Some(9));
        let names: Vec<_> = p.fields.iter().map(|f| f.0.as_str()).collect();
        assert_eq!(names, ["name", "email", "address", "tags.*"]);
    }

    #[test]
    fn rejects_non_requests_and_handles_junk() {
        assert!(parse("<?php\nclass User extends Model {}\n").is_none());
        assert!(parse("").is_none());
        let p = parse("class A extends FormRequest {\n function rules() { return []; }\n}").unwrap();
        assert!(p.fields.is_empty());
        let p = parse("class A extends FormRequest {\n function rules() {\n").unwrap();
        assert!(p.fields.is_empty());
        assert!(parse("class A extends FormRequest {\n function rules() {\n return [\n 'é日' => 1,\n 'x").unwrap().fields.len() == 1);
    }

    #[test]
    fn discovers_and_bounds() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let dir = r.join("app/Http/Requests/Admin");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("StoreUserRequest.php"), SAMPLE).unwrap();
        std::fs::write(dir.join("Helper.php"), "<?php\nclass Helper {}\n").unwrap();
        let big: String = (0..300).map(|i| format!("'f{i}' => 'x',\n")).collect();
        std::fs::write(dir.join("Big.php"), format!("class Big extends FormRequest {{\n function rules() {{\n return [\n{big}];\n }}\n}}")).unwrap();
        let got = discover(r);
        assert!(got.iter().any(|t| t.label.starts_with("StoreUserRequest  app/Http/Requests/Admin/")));
        assert!(got.iter().any(|t| t.label == "StoreUserRequest  rule: email" && t.line == 13));
        assert_eq!(got.iter().filter(|t| t.label.starts_with("Big  rule:")).count(), MAX_FIELDS);
        assert!(!got.iter().any(|t| t.label.starts_with("Helper")));
        assert!(discover(&r.join("nope")).is_empty());
    }

    #[test]
    fn random_text_never_panics() {
        let pieces = ["class A extends FormRequest {", "function rules", "return [", "'a' =>", "\"é\" =>", "]", "[", "(", ")", "'", "\"", "\\", "\n", " ", "//", "日本"];
        let mut s = 99u64;
        for _ in 0..500 {
            let mut t = String::new();
            for _ in 0..80 {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                t.push_str(pieces[(s >> 33) as usize % pieces.len()]);
            }
            let _ = parse(&t);
        }
    }
}
