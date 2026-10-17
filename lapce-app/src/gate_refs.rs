//! Go to a Laravel gate from its use: the ability name in `Gate::allows('x')`,
//! `$user->can('x')`, `$this->authorize('x')`, `@can('x')` or `'can:x'`
//! middleware opens `Gate::define('x'` in `app/Providers`.
//!
//! Only names that have a `Gate::define` are resolved, so policy abilities
//! such as `can('update', $post)` fall through to normal lookup. When a gate
//! and a policy share a name, the gate is opened. Textual only; nothing is
//! executed. Bounded like the policy index (depth 3, 500 files, 1 MB each).
use std::path::{Path, PathBuf};

const CALLERS: &[&str] = &[
    "Gate::allows", "Gate::denies", "Gate::check", "Gate::authorize", "Gate::inspect",
    "->can", "->cannot", "->authorize", "@can", "@cannot", "@canany", "@elsecan",
];

/// The quoted string containing byte `col`: `(contents, text_before_quote)`.
fn string_at(line: &str, col: usize) -> Option<(&str, &str)> {
    let mut open: Option<(char, usize)> = None;
    for (i, c) in line.char_indices() {
        match open {
            None if c == '\'' || c == '"' => open = Some((c, i)),
            Some((q, s)) if c == q => {
                if col > s && col <= i {
                    return Some((&line[s + 1..i], &line[..s]));
                }
                open = None;
            }
            _ => {}
        }
    }
    None
}

fn gate_name(line: &str, col: usize) -> Option<String> {
    let (s, before) = string_at(line, col)?;
    if s.is_empty() || s.len() > 100 {
        return None;
    }
    let head = before.trim_end().strip_suffix('(').map(str::trim_end);
    if let Some(h) = head {
        for c in CALLERS {
            if let Some(pre) = h.strip_suffix(c) {
                let ok = pre.chars().next_back().is_none_or(|p| !(p.is_ascii_alphanumeric() || p == '_'));
                if ok || c.starts_with("->") {
                    return Some(s.to_string());
                }
            }
        }
    }
    // `->middleware('can:name,model')`
    if before.contains("middleware") {
        let rest = s.strip_prefix("can:")?;
        let name = rest.split(',').next()?.trim();
        return (!name.is_empty()).then(|| name.to_string());
    }
    None
}

/// Gate definition file and 0-based line for the ability name under `col`.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let name = gate_name(line, col)?;
    crate::policy_index::find_gate(root, &name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    fn name(l: &str, n: &str) -> Option<String> {
        gate_name(l, at(l, n))
    }

    #[test]
    fn classifies_uses() {
        let some = |s: &str| Some(s.to_string());
        assert_eq!(name("if (Gate::allows('edit-settings')) {", "edit"), some("edit-settings"));
        assert_eq!(name("$this->authorize( \"edit-settings\" );", "edit"), some("edit-settings"));
        assert_eq!(name("auth()->user()->can('admin', $x)", "admin"), some("admin"));
        assert_eq!(name("@can('admin')", "admin"), some("admin"));
        assert_eq!(name("@canany('admin')", "admin"), some("admin"));
        assert_eq!(name("Route::get('/')->middleware('can:admin,post');", "admin"), some("admin"));
        assert_eq!(name("Route::get('/')->middleware('auth');", "auth"), None);
        assert_eq!(name("MyGate::allows('admin')", "admin"), None);
        assert_eq!(name("Gate::allows($x, 'admin')", "admin"), None);
        assert_eq!(name("foo('admin')", "admin"), None);
    }

    #[test]
    fn resolves_to_the_define_line() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let dir = r.join("app/Providers");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("AuthServiceProvider.php");
        std::fs::write(&f, "<?php\nclass A {\n  function boot() {\n    Gate::define('admin', fn () => true);\n  }\n}\n").unwrap();
        let l = "@can('admin')";
        assert_eq!(resolve(r, l, at(l, "admin")), Some((f, 3)));
        let l = "@can('update', $post)";
        assert_eq!(resolve(r, l, at(l, "update")), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        for l in ["@can('", "->can(\"é", "Gate::allows('日本')", "'can:é,x'->middleware(", "->middleware('can:", "(", "'", "@can('😀'", ""] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), l, c);
            }
        }
    }
}
