//! Go to the Livewire component member a Blade view refers to:
//! `wire:click="save"`, `wire:submit="save"`, `wire:model.live="form.name"`,
//! `wire:click="delete({{ $id }})"`, and Alpine `$wire.save()` /
//! `$wire.count` opens the `function save(` or the `public $count` line in
//! the component class.
//!
//! The class is found from the view path: `resources/views/livewire/admin/user-profile.blade.php`
//! -> `app/Livewire/Admin/UserProfile.php` (v3) or `app/Http/Livewire/...` (v2).
//! Textual only; nothing is executed. Magic actions (`$refresh`, `$set(...)`,
//! `$toggle(...)`) have no PHP definition and are skipped, as are attributes
//! that do not take a component member (`wire:key`, `wire:loading.class`).
//! Traits, parent classes and Volt single-file components are not searched.
use std::path::{Path, PathBuf};

const MAX_FILE: u64 = 1 << 20;
/// `wire:<event>` attributes whose value starts with a component member.
const EVENTS: &[&str] = &[
    "click", "submit", "model", "keydown", "keyup", "change", "input", "blur", "focus",
    "init", "poll", "intersect", "mouseenter", "mouseleave", "dblclick", "scroll",
    "paste", "select", "confirm",
];

fn studly(part: &str) -> String {
    part.split(['-', '_'])
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect()
}

fn class_for_view(root: &Path, view: &Path) -> Option<PathBuf> {
    let rel = view.strip_prefix(root.join("resources/views/livewire")).ok()?;
    let s = rel.to_str()?.strip_suffix(".blade.php")?;
    if s.is_empty() || s.contains("..") || s.split('/').any(str::is_empty) {
        return None;
    }
    let class: PathBuf = s.split('/').map(studly).collect::<Vec<_>>().join("/").into();
    for base in ["app/Livewire", "app/Http/Livewire"] {
        let p = root.join(base).join(&class).with_extension("php");
        if std::fs::symlink_metadata(&p).is_ok_and(|m| m.is_file() && m.len() <= MAX_FILE) {
            return Some(p);
        }
    }
    None
}

fn is_name(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn clamp_col(line: &str, col: usize) -> usize {
    let mut c = col.min(line.len());
    while !line.is_char_boundary(c) {
        c -= 1;
    }
    c
}

/// The member name under byte `col`, from a `wire:` attribute value or a
/// `$wire.` access.
pub fn member_at(line: &str, col: usize) -> Option<String> {
    let col = clamp_col(line, col);
    // `$wire.name`
    let mut from = 0;
    while let Some(p) = line[from..].find("$wire.") {
        let start = from + p + 6;
        let end = start + line[start..].find(|c: char| !is_name(c)).unwrap_or(line.len() - start);
        if end > start && col >= start && col <= end {
            return Some(line[start..end].to_string());
        }
        from = start;
    }
    // `wire:event.modifier="name..."`
    let mut from = 0;
    while let Some(p) = line[from..].find("wire:") {
        let at = from + p;
        from = at + 5;
        if line[..at].chars().next_back().is_some_and(|c| is_name(c) || matches!(c, ':' | '-' | '.')) {
            continue;
        }
        let after = &line[at + 5..];
        let attr_len = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
            .unwrap_or(after.len());
        let attr = &after[..attr_len];
        let event = attr.split('.').next().unwrap_or("");
        if !EVENTS.contains(&event) {
            continue;
        }
        let rest = &after[attr_len..];
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else { continue };
        let value_start = line.len() - rest.len() + 1;
        let Some(vlen) = line[value_start..].find(q) else { continue };
        let value = &line[value_start..value_start + vlen];
        let lead = value.len() - value.trim_start().len();
        let s = value_start + lead;
        let name_len = line[s..value_start + vlen].find(|c: char| !is_name(c)).unwrap_or(value_start + vlen - s);
        // `$refresh`, `$set(...)`: a `$` start is not a member.
        if name_len > 0 && col >= s && col <= s + name_len {
            return Some(line[s..s + name_len].to_string());
        }
    }
    None
}

/// Zero-based line of the method or public property `name` in `text`.
pub fn find_member(text: &str, name: &str) -> Option<usize> {
    let fpat = format!("function {name}");
    let fpat_ref = format!("function &{name}");
    let method = text.lines().position(|l| {
        let t = l.trim_start();
        if t.starts_with("//") || t.starts_with('*') || t.starts_with('#') {
            return false;
        }
        [&fpat, &fpat_ref].iter().any(|p| {
            l.match_indices(p.as_str()).any(|(i, _)| {
                let before_ok = l[..i].chars().next_back().is_none_or(|c| !is_name(c));
                let after = l[i + p.len()..].chars().next();
                before_ok && after.is_some_and(|c| c == '(' || c.is_whitespace())
            })
        }) && !t.starts_with("private ")
            && !t.starts_with("protected ")
    });
    if method.is_some() {
        return method;
    }
    let ppat = format!("${name}");
    text.lines().position(|l| {
        let t = l.trim_start();
        let vis = t.starts_with("public ") || t.starts_with("#[");
        vis && !t.contains('(')
            && t.match_indices(&ppat).any(|(i, _)| {
                t[i + ppat.len()..].chars().next().is_none_or(|c| !is_name(c))
            })
    })
}

/// Target for the member under `col` of `line` in the Livewire view `file`.
pub fn resolve(root: &Path, file: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    if line.len() > 8192 {
        return None;
    }
    let name = member_at(line, col)?;
    let class = class_for_view(root, file)?;
    let text = std::fs::read_to_string(&class).ok()?;
    let l = find_member(&text, &name)?;
    Some((class, l))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLASS: &str = "<?php
namespace App\\Livewire\\Admin;

class UserProfile extends Component
{
    public string $email = '';
    #[Locked] public int $count = 0;
    public array $form = [];

    private function hidden() {}

    public function save(): void
    {
    }

    public function delete($id) {}
}
";

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    fn project() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let class = r.join("app/Livewire/Admin/UserProfile.php");
        std::fs::create_dir_all(class.parent().unwrap()).unwrap();
        std::fs::write(&class, CLASS).unwrap();
        let view = r.join("resources/views/livewire/admin/user-profile.blade.php");
        std::fs::create_dir_all(view.parent().unwrap()).unwrap();
        std::fs::write(&view, "x").unwrap();
        (d, view, class)
    }

    #[test]
    fn wire_attributes_and_wire_access() {
        let (d, view, class) = project();
        let r = d.path();
        let go = |l: &str, n: &str| resolve(r, &view, l, at(l, n));
        let l = "<button wire:click=\"save\">Save</button>";
        assert_eq!(go(l, "save"), Some((class.clone(), 11)));
        let l = "<form wire:submit.prevent='save'>";
        assert_eq!(go(l, "save"), Some((class.clone(), 11)));
        let l = "<input wire:model.live=\"email\">";
        assert_eq!(go(l, "email"), Some((class.clone(), 5)));
        let l = "<input wire:model=\"form.name\">";
        assert_eq!(go(l, "form"), Some((class.clone(), 7)));
        let l = "<a wire:click=\"delete({{ $id }})\">x</a>";
        assert_eq!(go(l, "delete"), Some((class.clone(), 15)));
        let l = "<div x-on:click=\"$wire.save()\" x-text=\"$wire.count\">";
        assert_eq!(go(l, "save"), Some((class.clone(), 11)));
        assert_eq!(go(l, "count"), Some((class.clone(), 6)));
    }

    #[test]
    fn skips_non_members() {
        let (d, view, _) = project();
        let r = d.path();
        let go = |l: &str, n: &str| resolve(r, &view, l, at(l, n));
        let l = "<button wire:click=\"$refresh\">";
        assert_eq!(go(l, "refresh"), None);
        let l = "<button wire:click=\"$set('email', 'x')\">";
        assert_eq!(go(l, "email"), None);
        let l = "<div wire:loading.class=\"save\">";
        assert_eq!(go(l, "save"), None);
        let l = "<div wire:key=\"save\">";
        assert_eq!(go(l, "save"), None);
        let l = "<button data-wire:click=\"save\">";
        assert_eq!(go(l, "save"), None);
        let l = "<button wire:click=\"hidden\">";
        assert_eq!(go(l, "hidden"), None);
        let l = "<button wire:click=\"missing\">";
        assert_eq!(go(l, "missing"), None);
        // Not a Livewire view.
        let other = r.join("resources/views/welcome.blade.php");
        let l = "<button wire:click=\"save\">";
        assert_eq!(resolve(r, &other, l, at(l, "save")), None);
    }

    #[test]
    fn hostile_lines_never_panic() {
        let (d, view, _) = project();
        let mut x: u64 = 0x0f0f_1234_dead_beef;
        let alpha: Vec<char> = "wire:click=\"'$.(){ }é日save\\ ".chars().collect();
        for _ in 0..500 {
            let mut s = String::new();
            for _ in 0..80 {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                s.push(alpha[(x % alpha.len() as u64) as usize]);
            }
            for c in [0, 7, 30, 79, 400] {
                let _ = resolve(d.path(), &view, &s, c);
            }
        }
        assert_eq!(resolve(d.path(), &view, &"a".repeat(9000), 5), None);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_class_is_not_followed() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::create_dir_all(r.join("app/Livewire")).unwrap();
        std::fs::write(r.join("outside.php"), CLASS).unwrap();
        std::os::unix::fs::symlink(r.join("outside.php"), r.join("app/Livewire/Foo.php")).unwrap();
        let view = r.join("resources/views/livewire/foo.blade.php");
        let l = "<b wire:click=\"save\">";
        assert_eq!(resolve(r, &view, l, at(l, "save")), None);
    }
}
