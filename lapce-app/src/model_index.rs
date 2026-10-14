//! Index for the "Go to Model" palette: Eloquent models under `app/Models`
//! (or `app/` when that folder is missing) and their relationships.
//!
//! A class counts as a model when it `extends Model`, `Authenticatable`,
//! `Pivot` or `MorphPivot`. A relationship is a method whose body starts with
//! `return $this->hasOne|hasMany|belongsTo|belongsToMany|hasOneThrough|
//! hasManyThrough|morphOne|morphMany|morphTo|morphToMany|morphedByMany(...)`.
//! Static text scan, nothing is executed. Bounded: depth 6, 1500 files,
//! 1 MB per file, 5000 entries, symlinks skipped.
use std::path::{Path, PathBuf};

use crate::route_index::RouteTarget;

const MAX_FILE: u64 = 1 << 20;
const MAX_FILES: usize = 1500;
const MAX_DEPTH: usize = 6;
const MAX_ENTRIES: usize = 5000;

const RELATIONS: &[&str] = &[
    "hasOne", "hasMany", "belongsTo", "belongsToMany", "hasOneThrough", "hasManyThrough",
    "morphOne", "morphMany", "morphTo", "morphToMany", "morphedByMany",
];
const PARENTS: &[&str] = &["Model", "Authenticatable", "Pivot", "MorphPivot"];

#[derive(Debug, PartialEq, Eq)]
pub struct Parsed {
    pub class: String,
    pub class_line: usize,
    /// `(method, relation kind, related class as written, 0-based line)`.
    pub relations: Vec<(String, String, String, usize)>,
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

/// Parse one PHP file; `None` when it declares no model class.
pub fn parse(text: &str) -> Option<Parsed> {
    let mut class: Option<(String, usize)> = None;
    let mut relations = Vec::new();
    let mut method: Option<(String, usize)> = None;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with("//") || line.starts_with('*') || line.starts_with('#') {
            continue;
        }
        if class.is_none() {
            if line.starts_with("class ")
                || line.starts_with("final class ")
                || line.starts_with("abstract class ")
            {
                let name = ident_after(line, "class ")?;
                let parent = ident_after(line, " extends ")?;
                let parent = parent.rsplit('\\').next().unwrap_or(parent);
                if PARENTS.contains(&parent) {
                    class = Some((name.to_string(), i));
                } else {
                    return None;
                }
            }
            continue;
        }
        if let Some(pos) = line.find("function ") {
            let before = &line[..pos];
            if before.is_empty()
                || before.split_whitespace().all(|w| matches!(w, "public" | "protected" | "private" | "static" | "final"))
            {
                if let Some(name) = ident_after(line, "function ") {
                    method = Some((name.to_string(), i));
                }
            }
        }
        if let Some(p) = line.find("$this->") {
            if !line.starts_with("return") {
                continue;
            }
            let rest = &line[p + 7..];
            for kind in RELATIONS {
                if let Some(args) = rest.strip_prefix(kind).and_then(|r| r.strip_prefix('(')) {
                    if let Some((m, ml)) = method.take() {
                        let related = args
                            .split("::class")
                            .next()
                            .filter(|_| args.contains("::class"))
                            .map(|s| s.trim().trim_matches(|c| c == '\'' || c == '"').rsplit('\\').next().unwrap_or("").to_string())
                            .unwrap_or_default();
                        relations.push((m, kind.to_string(), related, ml));
                    }
                    break;
                }
            }
        }
    }
    class.map(|(class, class_line)| Parsed { class, class_line, relations })
}

pub fn discover(root: &Path) -> Vec<RouteTarget> {
    let base = if std::fs::symlink_metadata(root.join("app/Models")).is_ok() {
        root.join("app/Models")
    } else {
        root.join("app")
    };
    let mut files = Vec::new();
    walk(&base, 0, &mut files);
    let mut out = Vec::new();
    for f in files {
        let Ok(md) = std::fs::symlink_metadata(&f) else { continue };
        if !md.is_file() || md.len() > MAX_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let Some(p) = parse(&text) else { continue };
        let shown = f.strip_prefix(root).unwrap_or(&f).display().to_string();
        out.push(RouteTarget {
            label: format!("model  {}  ({shown})", p.class),
            path: f.clone(),
            line: p.class_line,
        });
        for (m, kind, related, line) in p.relations {
            let to = if related.is_empty() { String::new() } else { format!(" {related}") };
            out.push(RouteTarget {
                label: format!("relation  {}::{m}  {kind}{to}", p.class),
                path: f.clone(),
                line,
            });
        }
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    out.truncate(MAX_ENTRIES);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER: &str = "<?php\nnamespace App\\Models;\n\nuse Illuminate\\Database\\Eloquent\\Model;\n\nclass User extends Authenticatable\n{\n    // public function ghost() { return $this->hasMany(X::class); }\n    public function posts(): HasMany\n    {\n        return $this->hasMany(Post::class);\n    }\n\n    public function team()\n    {\n        return $this->belongsTo(\\App\\Models\\Team::class, 'team_id');\n    }\n\n    public function name() { return $this->first . $this->last; }\n    public function taggable()\n    {\n        return $this->morphTo();\n    }\n}\n";

    #[test]
    fn parses_model_and_relations() {
        let p = parse(USER).unwrap();
        assert_eq!(p.class, "User");
        assert_eq!(p.class_line, 5);
        assert_eq!(
            p.relations,
            vec![
                ("posts".into(), "hasMany".into(), "Post".into(), 8),
                ("team".into(), "belongsTo".into(), "Team".into(), 13),
                ("taggable".into(), "morphTo".into(), "".into(), 19),
            ]
        );
    }

    #[test]
    fn non_models_are_skipped() {
        assert!(parse("<?php\nclass Helper\n{\n}\n").is_none());
        assert!(parse("<?php\nclass A extends Controller {}\n").is_none());
        assert!(parse("<?php\ninterface X {}\n").is_none());
        assert!(parse("").is_none());
        assert!(parse("class").is_none());
        assert!(parse("final class Post extends Model {}").is_some());
    }

    #[test]
    fn discover_end_to_end() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::create_dir_all(r.join("app/Models/Billing")).unwrap();
        std::fs::write(r.join("app/Models/User.php"), USER).unwrap();
        std::fs::write(r.join("app/Models/Billing/Invoice.php"), "<?php\nclass Invoice extends Model\n{\n}\n").unwrap();
        std::fs::write(r.join("app/Models/NotAModel.php"), "<?php\nclass NotAModel {}\n").unwrap();
        let got = discover(r);
        let labels: Vec<_> = got.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels[0], "model  Invoice  (app/Models/Billing/Invoice.php)");
        assert!(labels.contains(&"relation  User::posts  hasMany Post"));
        assert!(labels.contains(&"relation  User::taggable  morphTo"));
        assert_eq!(labels.len(), 5);
        assert!(discover(&r.join("nope")).is_empty());
    }

    #[test]
    fn never_panics_and_is_bounded() {
        for t in ["é", "class é extends Model {", "function \u{1F600}", "return $this->hasMany(", "class A extends"] {
            let _ = parse(t);
        }
        let big = format!("<?php\nclass A extends Model {{\n{}}}\n", "public function x() { return $this->hasMany(B::class); }\n".repeat(50_000));
        let start = std::time::Instant::now();
        assert!(parse(&big).is_some());
        assert!(start.elapsed().as_secs() < 5);
    }
}
