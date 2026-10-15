//! Index for the "Livewire: Find Member Usages" palette.
//!
//! From the active Livewire component class (`app/Livewire/Admin/UserProfile.php`)
//! or its view, lists where the view uses the class's methods and properties:
//! `wire:click="save"`, `wire:submit.prevent="save"`, `wire:model.live="form.name"`,
//! `wire:click="delete({{ $id }})"` and Alpine `$wire.save()` / `$wire.count`.
//! Each entry opens the view at that line:
//! `save  resources/views/livewire/admin/user-profile.blade.php:12  <button wire:click="save">`.
//!
//! Only names the class declares (`function name(` or a `public` property) are
//! listed, so `wire:loading.class` values and `$refresh` do not appear. The view
//! is found by mapping the class path to a kebab-case view path
//! (`UserProfile` -> `user-profile`). Static text scan, nothing is executed.
//! Bounded: 1 MB per file, 8 KB per line, 500 entries, symlinks not followed.
//! Other views that embed the component, traits and parent classes are not scanned.
use std::path::{Path, PathBuf};

use crate::route_index::RouteTarget;

const MAX_FILE: u64 = 1 << 20;
const MAX_LINE: usize = 8192;
const MAX_ENTRIES: usize = 500;
const EVENTS: &[&str] = &[
    "click", "submit", "model", "keydown", "keyup", "change", "input", "blur", "focus",
    "init", "poll", "intersect", "mouseenter", "mouseleave", "dblclick", "scroll",
    "paste", "select", "confirm",
];

fn is_name(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn kebab(seg: &str) -> String {
    let mut out = String::new();
    for (i, c) in seg.chars().enumerate() {
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

fn studly(part: &str) -> String {
    part.split(['-', '_'])
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect()
}

fn plain_file(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_file() && m.len() <= MAX_FILE)
}

/// `(class file, view file)` for a Livewire class or view path.
pub fn pair(root: &Path, file: &Path) -> Option<(PathBuf, PathBuf)> {
    for base in ["app/Livewire", "app/Http/Livewire"] {
        if let Ok(rel) = file.strip_prefix(root.join(base)) {
            let s = rel.to_str()?.strip_suffix(".php")?;
            if s.is_empty() || s.contains("..") {
                return None;
            }
            let view: PathBuf = s.split('/').map(kebab).collect::<Vec<_>>().join("/").into();
            let view = root
                .join("resources/views/livewire")
                .join(view)
                .with_extension("blade.php");
            return (plain_file(file) && plain_file(&view)).then(|| (file.to_path_buf(), view));
        }
    }
    let rel = file.strip_prefix(root.join("resources/views/livewire")).ok()?;
    let s = rel.to_str()?.strip_suffix(".blade.php")?;
    if s.is_empty() || s.contains("..") || s.split('/').any(str::is_empty) {
        return None;
    }
    let class: PathBuf = s.split('/').map(studly).collect::<Vec<_>>().join("/").into();
    for base in ["app/Livewire", "app/Http/Livewire"] {
        let c = root.join(base).join(&class).with_extension("php");
        if plain_file(&c) && plain_file(file) {
            return Some((c, file.to_path_buf()));
        }
    }
    None
}

/// Names used on `line`, each member's first segment (`form.name` -> `form`).
pub fn used_names(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    if line.len() > MAX_LINE {
        return out;
    }
    let mut from = 0;
    while let Some(p) = line[from..].find("$wire.") {
        let s = from + p + 6;
        let e = s + line[s..].find(|c: char| !is_name(c)).unwrap_or(line.len() - s);
        if e > s {
            out.push(line[s..e].to_string());
        }
        from = s;
    }
    let mut from = 0;
    while let Some(p) = line[from..].find("wire:") {
        let at = from + p;
        from = at + 5;
        if line[..at].chars().next_back().is_some_and(|c| is_name(c) || matches!(c, ':' | '-' | '.')) {
            continue;
        }
        let after = &line[at + 5..];
        let alen = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
            .unwrap_or(after.len());
        if !EVENTS.contains(&after[..alen].split('.').next().unwrap_or("")) {
            continue;
        }
        let Some(rest) = after[alen..].strip_prefix('=') else { continue };
        let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else { continue };
        let Some(vlen) = rest[1..].find(q) else { continue };
        let value = rest[1..1 + vlen].trim_start();
        let n: String = value.chars().take_while(|c| is_name(*c)).collect();
        if !n.is_empty() {
            out.push(n);
        }
    }
    out
}

/// Does the class text declare `name` as a method or public property?
pub fn declares(text: &str, name: &str) -> bool {
    let f = format!("function {name}");
    let fr = format!("function &{name}");
    let p = format!("${name}");
    text.lines().any(|l| {
        let t = l.trim_start();
        if t.starts_with("//") || t.starts_with('*') {
            return false;
        }
        let m = [&f, &fr].iter().any(|pat| {
            l.match_indices(pat.as_str()).any(|(i, _)| {
                l[..i].chars().next_back().is_none_or(|c| !is_name(c))
                    && l[i + pat.len()..].chars().next().is_some_and(|c| c == '(' || c.is_whitespace())
            })
        }) && !t.starts_with("private ")
            && !t.starts_with("protected ");
        let prop = (t.starts_with("public ") || t.starts_with("#["))
            && !t.contains('(')
            && t.match_indices(&p).any(|(i, _)| t[i + p.len()..].chars().next().is_none_or(|c| !is_name(c)));
        m || prop
    })
}

pub fn discover(root: &Path, file: &Path) -> Vec<RouteTarget> {
    let Some((class, view)) = pair(root, file) else { return Vec::new() };
    let (Ok(ctext), Ok(vtext)) = (std::fs::read_to_string(&class), std::fs::read_to_string(&view)) else {
        return Vec::new();
    };
    let shown = view.strip_prefix(root).unwrap_or(&view).display().to_string();
    let mut out = Vec::new();
    for (i, l) in vtext.lines().enumerate() {
        for n in used_names(l) {
            if out.len() >= MAX_ENTRIES {
                return out;
            }
            if declares(&ctext, &n) {
                let snippet: String = l.trim().chars().take(100).collect();
                out.push(RouteTarget {
                    label: format!("{n}  {shown}:{}  {snippet}", i + 1),
                    path: view.clone(),
                    line: i,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLASS: &str = "<?php
class UserProfile extends Component
{
    public string $email = '';
    public array $form = [];
    private $hidden;
    public function save() {}
    public function delete($id) {}
}
";

    const VIEW: &str = "<div>
  <input wire:model.live=\"form.name\">
  <input wire:model=\"email\">
  <button wire:click=\"save\">Save</button>
  <button wire:click=\"delete({{ $id }})\">x</button>
  <span x-text=\"$wire.email\"></span>
  <div wire:loading.class=\"save\"></div>
  <button wire:click=\"$refresh\">r</button>
  <button wire:click=\"hidden\">h</button>
  <button wire:click=\"missing\">m</button>
</div>
";

    fn project() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let class = r.join("app/Livewire/Admin/UserProfile.php");
        let view = r.join("resources/views/livewire/admin/user-profile.blade.php");
        for (p, t) in [(&class, CLASS), (&view, VIEW)] {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, t).unwrap();
        }
        (d, class, view)
    }

    #[test]
    fn lists_usages_from_class_or_view() {
        let (d, class, view) = project();
        for f in [&class, &view] {
            let items = discover(d.path(), f);
            let got: Vec<(String, usize)> = items
                .iter()
                .map(|t| (t.label.split_whitespace().next().unwrap().to_string(), t.line))
                .collect();
            assert_eq!(
                got,
                [("form".into(), 1), ("email".into(), 2), ("save".into(), 3), ("delete".into(), 4), ("email".into(), 5)]
            );
            assert!(items.iter().all(|t| t.path == view));
            assert!(items[2].label.contains("user-profile.blade.php:4"));
        }
    }

    #[test]
    fn unrelated_files_and_missing_view() {
        let (d, _, _) = project();
        let r = d.path();
        assert!(discover(r, &r.join("app/Models/User.php")).is_empty());
        std::fs::create_dir_all(r.join("app/Livewire")).unwrap();
        std::fs::write(r.join("app/Livewire/Lonely.php"), CLASS).unwrap();
        assert!(discover(r, &r.join("app/Livewire/Lonely.php")).is_empty());
    }

    #[test]
    fn name_extraction_cases() {
        assert_eq!(used_names("<a wire:click.prevent='go'>"), ["go"]);
        assert_eq!(used_names("x-on:click=\"$wire.a(); $wire.b = 1\""), ["a", "b"]);
        assert!(used_names("<div wire:key=\"x\" data-wire:click=\"y\">").is_empty());
        assert!(used_names(&format!("wire:click=\"a\"{}", " ".repeat(9000))).is_empty());
    }

    #[test]
    fn caps_and_fuzz() {
        let (d, class, view) = project();
        std::fs::write(&view, "<b wire:click=\"save\"></b>\n".repeat(900)).unwrap();
        assert_eq!(discover(d.path(), &class).len(), MAX_ENTRIES);
        let mut x: u64 = 0x1357_9bdf_2468_ace0;
        let alpha: Vec<char> = "wire:click=\"'$.(){ }é日save\\ ".chars().collect();
        for _ in 0..500 {
            let mut s = String::new();
            for _ in 0..80 {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                s.push(alpha[(x % alpha.len() as u64) as usize]);
            }
            let _ = used_names(&s);
            let _ = declares(&s, "save");
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_view_is_not_followed() {
        let (d, class, view) = project();
        std::fs::remove_file(&view).unwrap();
        std::fs::write(d.path().join("outside.php"), VIEW).unwrap();
        std::os::unix::fs::symlink(d.path().join("outside.php"), &view).unwrap();
        assert!(discover(d.path(), &class).is_empty());
    }
}
