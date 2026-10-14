//! Index of Blade, Livewire and Twig-less project components for the
//! "Go to Blade Component" palette: `<x-...>` anonymous (view) components,
//! `<x-...>` class components and `<livewire:...>` components.
//!
//! File names only, nothing is read or executed. Bounded: depth 8, 5000
//! files, symlinks not followed.
use std::path::{Path, PathBuf};

use crate::route_index::RouteTarget;

const MAX_ENTRIES: usize = 5000;
const MAX_DEPTH: usize = 8;

/// `UserProfile` -> `user-profile`.
fn kebab(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_ENTRIES {
        return;
    }
    if depth == 0
        && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink())
    {
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
        } else if ft.is_file() {
            out.push(p);
        }
        if out.len() >= MAX_ENTRIES {
            return;
        }
    }
}

/// Component name for `rel` (path under the base dir, `/` separated), given
/// how the file is named: strips the suffix, `index` collapses to its folder.
fn view_name(rel: &str) -> Option<String> {
    let stem = rel.strip_suffix(".blade.php")?;
    let stem = stem.strip_suffix("/index").unwrap_or(stem);
    (!stem.is_empty() && stem != "index").then(|| stem.replace('/', "."))
}

fn class_name(rel: &str) -> Option<String> {
    let stem = rel.strip_suffix(".php")?;
    if stem.ends_with(".blade") {
        return None;
    }
    Some(stem.split('/').map(kebab).collect::<Vec<_>>().join("."))
}

fn rel_of(base: &Path, p: &Path) -> Option<String> {
    let r = p.strip_prefix(base).ok()?;
    let parts: Vec<&str> = r.components().filter_map(|c| c.as_os_str().to_str()).collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Components of the project at `root`, sorted by name. A component that has
/// both a view and a class lists the view, as go-to does.
pub fn discover(root: &Path) -> Vec<RouteTarget> {
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    let mut files = Vec::new();

    // Anonymous components, then class components.
    let views = root.join("resources/views/components");
    walk(&views, 0, &mut files);
    for p in files.drain(..) {
        if let Some(n) = rel_of(&views, &p).and_then(|r| view_name(&r)) {
            found.push((format!("x-{n}"), p));
        }
    }
    let classes = root.join("app/View/Components");
    walk(&classes, 0, &mut files);
    for p in files.drain(..) {
        if let Some(n) = rel_of(&classes, &p).and_then(|r| class_name(&r)) {
            let key = format!("x-{n}");
            if !found.iter().any(|(k, _)| *k == key) {
                found.push((key, p));
            }
        }
    }

    // Livewire: classes (v3, v2), then views without a class.
    for base in ["app/Livewire", "app/Http/Livewire"] {
        let b = root.join(base);
        walk(&b, 0, &mut files);
        for p in files.drain(..) {
            if let Some(n) = rel_of(&b, &p).and_then(|r| class_name(&r)) {
                let key = format!("livewire:{n}");
                if !found.iter().any(|(k, _)| *k == key) {
                    found.push((key, p));
                }
            }
        }
    }
    let lv = root.join("resources/views/livewire");
    walk(&lv, 0, &mut files);
    for p in files.drain(..) {
        if let Some(n) = rel_of(&lv, &p).and_then(|r| view_name(&r)) {
            let key = format!("livewire:{n}");
            if !found.iter().any(|(k, _)| *k == key) {
                found.push((key, p));
            }
        }
    }

    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.truncate(MAX_ENTRIES);
    found
        .into_iter()
        .map(|(name, path)| {
            let shown = path.strip_prefix(root).unwrap_or(&path).display().to_string();
            RouteTarget { label: format!("<{name}>  {shown}"), path, line: 0 }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(root: &Path, rel: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "x").unwrap();
    }

    #[test]
    fn names() {
        assert_eq!(view_name("forms/input.blade.php").as_deref(), Some("forms.input"));
        assert_eq!(view_name("card/index.blade.php").as_deref(), Some("card"));
        assert_eq!(view_name("index.blade.php"), None);
        assert_eq!(view_name("a.php"), None);
        assert_eq!(class_name("Forms/DateInput.php").as_deref(), Some("forms.date-input"));
        assert_eq!(class_name("a.blade.php"), None);
        assert_eq!(kebab("UserProfile"), "user-profile");
    }

    #[test]
    fn discovers_all_kinds_and_prefers_views() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        touch(r, "resources/views/components/forms/input.blade.php");
        touch(r, "resources/views/components/card/index.blade.php");
        touch(r, "app/View/Components/Forms/Input.php");
        touch(r, "app/View/Components/Alert.php");
        touch(r, "app/Livewire/Admin/UserProfile.php");
        touch(r, "resources/views/livewire/admin/user-profile.blade.php");
        touch(r, "resources/views/livewire/counter.blade.php");
        let got = discover(r);
        let names: Vec<_> = got.iter().map(|t| t.label.split_whitespace().next().unwrap().to_string()).collect();
        assert_eq!(
            names,
            ["<livewire:admin.user-profile>", "<livewire:counter>", "<x-alert>", "<x-card>", "<x-forms.input>"]
        );
        let input = got.iter().find(|t| t.label.starts_with("<x-forms.input>")).unwrap();
        assert!(input.path.ends_with("input.blade.php"));
        let lw = got.iter().find(|t| t.label.starts_with("<livewire:admin")).unwrap();
        assert!(lw.path.ends_with("UserProfile.php"));
    }

    #[test]
    fn empty_project_and_symlinks() {
        let d = tempfile::tempdir().unwrap();
        assert!(discover(d.path()).is_empty());
        #[cfg(unix)]
        {
            let real = d.path().join("real");
            std::fs::create_dir_all(&real).unwrap();
            touch(&real, "a.blade.php");
            let c = d.path().join("resources/views");
            std::fs::create_dir_all(&c).unwrap();
            std::os::unix::fs::symlink(&real, c.join("components")).unwrap();
            assert!(discover(d.path()).is_empty());
        }
    }
}
