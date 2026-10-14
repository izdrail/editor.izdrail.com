//! Index for the "Go to Policy / Gate" palette: Laravel policies under
//! `app/Policies` (class, abilities and the model each ability takes) and
//! `Gate::define('name', ...)` calls in `app/Providers`.
//!
//! An ability is a public method other than the constructor, `before` and
//! `after`. Static text scan, nothing is executed. Bounded: depth 3, 500
//! files, 1 MB per file, 5000 entries, symlinks skipped.
use std::path::{Path, PathBuf};

use crate::route_index::RouteTarget;

const MAX_FILE: u64 = 1 << 20;
const MAX_FILES: usize = 500;
const MAX_DEPTH: usize = 3;
const MAX_ENTRIES: usize = 5000;

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

fn read(path: &Path) -> Option<String> {
    let md = std::fs::symlink_metadata(path).ok()?;
    if !md.is_file() || md.len() > MAX_FILE {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

fn ident(s: &str) -> Option<&str> {
    let end = s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(s.len());
    (end > 0).then(|| &s[..end])
}

#[derive(Debug, PartialEq, Eq)]
pub struct Policy {
    pub class: String,
    pub class_line: usize,
    /// `(ability, model type of the second parameter if any, 0-based line)`.
    pub abilities: Vec<(String, Option<String>, usize)>,
}

/// Parse a policy class; `None` when the file has no class.
pub fn parse_policy(text: &str) -> Option<Policy> {
    let mut class: Option<(String, usize)> = None;
    let mut abilities = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with("//") || line.starts_with('*') || line.starts_with('#') {
            continue;
        }
        if class.is_none() {
            if let Some(rest) = line
                .strip_prefix("class ")
                .or_else(|| line.strip_prefix("final class "))
                .or_else(|| line.strip_prefix("abstract class "))
            {
                class = ident(rest).map(|n| (n.to_string(), i));
            }
            continue;
        }
        let Some(pos) = line.find("function ") else { continue };
        let before = &line[..pos];
        // Visibility: bare `function` means public in PHP.
        if !before.split_whitespace().all(|w| matches!(w, "public" | "final" | "static")) {
            continue;
        }
        let Some(name) = ident(&line[pos + 9..]) else { continue };
        if matches!(name, "__construct" | "before" | "after") {
            continue;
        }
        // Second parameter's type: `(User $user, Post $post)` -> Post.
        let params = line[pos + 9 + name.len()..].trim_start().strip_prefix('(').unwrap_or("");
        let model = params.split(',').nth(1).and_then(|p| {
            let t = p.split('$').next()?.trim().trim_start_matches('?');
            let t = t.rsplit('\\').next()?;
            (!t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then(|| t.to_string())
        });
        abilities.push((name.to_string(), model, i));
    }
    class.map(|(class, class_line)| Policy { class, class_line, abilities })
}

/// `(gate name, 0-based line)` for each `Gate::define('name'`.
pub fn parse_gates(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let mut rest = line;
        while let Some(p) = rest.find("Gate::define(") {
            rest = &rest[p + 13..];
            let a = rest.trim_start();
            if let Some(q) = a.chars().next().filter(|c| *c == '\'' || *c == '"') {
                let body = &a[1..];
                if let Some(end) = body.find(q) {
                    let n = &body[..end];
                    if !n.is_empty() && n.len() <= 100 && out.len() < 500 {
                        out.push((n.to_string(), i));
                    }
                }
            }
        }
    }
    out
}

pub fn discover(root: &Path) -> Vec<RouteTarget> {
    let mut out = Vec::new();
    let mut files = Vec::new();
    walk(&root.join("app/Policies"), 0, &mut files);
    for f in files.drain(..) {
        let Some(text) = read(&f) else { continue };
        let Some(p) = parse_policy(&text) else { continue };
        let names: Vec<&str> = p.abilities.iter().map(|(a, _, _)| a.as_str()).take(10).collect();
        let more = if p.abilities.len() > 10 { ", ..." } else { "" };
        out.push(RouteTarget {
            label: format!("policy  {}  ({}{more})", p.class, names.join(", ")),
            path: f.clone(),
            line: p.class_line,
        });
        for (a, model, line) in p.abilities {
            let m = model.map(|m| format!("  {m}")).unwrap_or_default();
            out.push(RouteTarget { label: format!("ability  {}::{a}{m}", p.class), path: f.clone(), line });
        }
        if out.len() >= MAX_ENTRIES {
            out.truncate(MAX_ENTRIES);
            return out;
        }
    }
    walk(&root.join("app/Providers"), 0, &mut files);
    for f in files {
        let Some(text) = read(&f) else { continue };
        let file = f.file_stem().and_then(|n| n.to_str()).unwrap_or("").to_string();
        for (name, line) in parse_gates(&text) {
            out.push(RouteTarget { label: format!("gate  {name}  ({file})"), path: f.clone(), line });
        }
    }
    out.truncate(MAX_ENTRIES);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: &str = "<?php\nnamespace App\\Policies;\n\nclass PostPolicy\n{\n    public function __construct() {}\n\n    public function before(User $u) {}\n\n    public function viewAny(User $user): bool\n    {\n        return true;\n    }\n\n    public function update(User $user, \\App\\Models\\Post $post): bool\n    {\n        return $user->id === $post->user_id;\n    }\n\n    protected function secret(User $u, Post $p) {}\n\n    function create(User $user, ?Team $team = null) {}\n}\n";

    #[test]
    fn parses_policy() {
        let p = parse_policy(POLICY).unwrap();
        assert_eq!(p.class, "PostPolicy");
        assert_eq!(p.class_line, 3);
        assert_eq!(
            p.abilities,
            vec![
                ("viewAny".to_string(), None, 9),
                ("update".to_string(), Some("Post".to_string()), 14),
                ("create".to_string(), Some("Team".to_string()), 21),
            ]
        );
        assert!(parse_policy("<?php\n").is_none());
    }

    #[test]
    fn parses_gates() {
        let t = "Gate::define('edit-settings', fn ($u) => true);\n  Gate::define(\"x\", [A::class, 'y']); Gate::define($n, 1);\n";
        assert_eq!(parse_gates(t), vec![("edit-settings".to_string(), 0), ("x".to_string(), 1)]);
    }

    #[test]
    fn discover_end_to_end() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::create_dir_all(r.join("app/Policies")).unwrap();
        std::fs::create_dir_all(r.join("app/Providers")).unwrap();
        std::fs::write(r.join("app/Policies/PostPolicy.php"), POLICY).unwrap();
        std::fs::write(r.join("app/Providers/AuthServiceProvider.php"), "<?php\nGate::define('admin', fn () => 1);\n").unwrap();
        let got = discover(r);
        let labels: Vec<_> = got.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels[0], "policy  PostPolicy  (viewAny, update, create)");
        assert!(labels.contains(&"ability  PostPolicy::update  Post"));
        assert!(labels.contains(&"gate  admin  (AuthServiceProvider)"));
        assert!(discover(&r.join("nope")).is_empty());
    }

    #[test]
    fn never_panics_and_is_fast() {
        for t in ["é", "class A { function é(", "class A { public function a(É $x, ", "Gate::define('é", "Gate::define("] {
            let _ = parse_policy(t);
            let _ = parse_gates(t);
        }
        let start = std::time::Instant::now();
        let _ = parse_policy(&format!("class A {{\n{}}}", "public function a(U $u, P $p) {}\n".repeat(200_000)));
        let _ = parse_gates(&"Gate::define('a', 1);".repeat(200_000));
        assert!(start.elapsed().as_secs() < 5);
    }
}
