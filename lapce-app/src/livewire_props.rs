//! Hover help for Livewire views: the word under the cursor is looked up in
//! the component class that renders the view.
//!
//! - `wire:model="email"`, `wire:click="save"`, `wire:model.live="form.name"`:
//!   the property or method named in the attribute value;
//! - `{{ $email }}` and other `$email` uses: the public property.
//!
//! The class is found from the view path: `resources/views/livewire/admin/user-profile.blade.php`
//! -> `app/Livewire/Admin/UserProfile.php` (v3) or `app/Http/Livewire/...` (v2).
//! Only `public` properties and methods are shown. Static text scan, nothing
//! is executed; `#[Computed]` and `mount()`-assigned properties are not
//! special-cased.
use std::path::{Path, PathBuf};

const MAX_FILE: u64 = 1 << 20;

fn studly(part: &str) -> String {
    part.split(['-', '_'])
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect()
}

/// Class file for a Livewire view path (relative parts after `views/livewire/`).
pub fn class_for_view(root: &Path, view: &Path) -> Option<PathBuf> {
    let rel = view.strip_prefix(root.join("resources/views/livewire")).ok()?;
    let s = rel.to_str()?.strip_suffix(".blade.php")?;
    if s.is_empty() || s.contains("..") {
        return None;
    }
    let class: PathBuf = s.split('/').map(studly).collect::<Vec<_>>().join("/").into();
    for base in ["app/Livewire", "app/Http/Livewire"] {
        let p = root.join(base).join(&class).with_extension("php");
        if std::fs::symlink_metadata(&p).is_ok_and(|m| m.is_file()) {
            return Some(p);
        }
    }
    None
}

fn is_name(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// What the cursor is on: a property/method name.
#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    /// `$name` use, or a `wire:` value that may be either.
    Property(String),
    /// A name inside a `wire:` attribute value.
    WireValue(String),
}

/// The name under byte `col` of `line`.
pub fn target_at(line: &str, col: usize) -> Option<Target> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let mut s = col;
    while let Some(c) = line[..s].chars().next_back() {
        if is_name(c) { s -= c.len_utf8() } else { break }
    }
    let mut e = col;
    while let Some(c) = line[e..].chars().next() {
        if is_name(c) { e += c.len_utf8() } else { break }
    }
    if s == e {
        return None;
    }
    let word = &line[s..e];
    if word.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    if line[..s].ends_with('$') {
        return Some(Target::Property(word.to_string()));
    }
    // Inside the quotes of a `wire:...="..."` value, on the first segment
    // (`form.name` -> `form`; later segments are fields of a form object).
    let before = &line[..s];
    let attr = before.rfind("wire:")?;
    let after_attr = &before[attr..];
    let eq = after_attr.find("=\"").or_else(|| after_attr.find("='"))?;
    let value = &after_attr[eq + 2..];
    if value.contains('"') || value.contains('\'') {
        return None; // the quote closed before the cursor
    }
    // Only the leading name of the expression, not `.field` or `->x`.
    if !value.trim().is_empty() && !value.trim_start_matches(['!', ' ', '$']).is_empty() {
        return None;
    }
    Some(Target::WireValue(word.to_string()))
}

/// `(0-based line, trimmed declaration)` of `public ... $name` or
/// `public function name(` in the class text.
pub fn find_member(text: &str, name: &str, want_method: Option<bool>) -> Option<(usize, String, bool)> {
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if !line.starts_with("public") {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.get(1) == Some(&"function") || words.iter().take(3).any(|w| *w == "function") {
            if want_method != Some(false) {
                if let Some(p) = line.find("function ") {
                    let rest = &line[p + 9..];
                    let end = rest.find(|c: char| !is_name(c)).unwrap_or(rest.len());
                    if &rest[..end] == name {
                        return Some((i, line.to_string(), true));
                    }
                }
            }
        } else if want_method != Some(true) {
            let needle = format!("${name}");
            if let Some(p) = line.find(&needle) {
                let next = line[p + needle.len()..].chars().next();
                if !next.is_some_and(is_name) {
                    return Some((i, line.to_string(), false));
                }
            }
        }
    }
    None
}

/// Markdown for the hover, when `file` is a Livewire view with a class.
pub fn help(root: &Path, file: &Path, line: &str, col: usize) -> Option<String> {
    let class = class_for_view(root, file)?;
    let target = target_at(line, col)?;
    let md = std::fs::symlink_metadata(&class).ok()?;
    if md.len() > MAX_FILE {
        return None;
    }
    let text = std::fs::read_to_string(&class).ok()?;
    let (name, want) = match &target {
        Target::Property(n) => (n.as_str(), Some(false)),
        Target::WireValue(n) => (n.as_str(), None),
    };
    let (n, decl, method) = find_member(&text, name, want)?;
    let shown = class.strip_prefix(root).unwrap_or(&class).display();
    let kind = if method { "method" } else { "property" };
    Some(format!("**Livewire {kind}** `{name}`\n```php\n{decl}\n```\n{shown}:{}", n + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str, m: &str) -> usize {
        s.find(m).unwrap() + 1
    }

    #[test]
    fn targets() {
        let l = r#"<input wire:model.live="email" wire:click="save(1)"> {{ $count }}"#;
        assert_eq!(target_at(l, at(l, "email")), Some(Target::WireValue("email".into())));
        assert_eq!(target_at(l, at(l, "save")), Some(Target::WireValue("save".into())));
        assert_eq!(target_at(l, at(l, "count")), Some(Target::Property("count".into())));
        assert_eq!(target_at(l, at(l, "input")), None);
        assert_eq!(target_at(l, at(l, "live")), None);
        assert_eq!(target_at(r#"wire:click="a->b""#, 14), None);
        assert_eq!(target_at(r#"wire:model="x" class="y""#, at(r#"wire:model="x" class="y""#, "y\"")), None);
        assert_eq!(target_at("$", 1), None);
        assert_eq!(target_at("é$日", 3), None);
    }

    #[test]
    fn members() {
        let t = "<?php\nclass A extends Component\n{\n    public string $email = '';\n    public array $items = [];\n    private $secret;\n\n    public function save(): void {}\n    public function mount($id) {}\n}\n";
        assert_eq!(find_member(t, "email", None).map(|m| m.0), Some(3));
        assert_eq!(find_member(t, "save", None).map(|m| (m.0, m.2)), Some((7, true)));
        assert_eq!(find_member(t, "save", Some(false)), None);
        assert_eq!(find_member(t, "secret", None), None);
        assert_eq!(find_member(t, "item", None), None);
        assert_eq!(find_member(t, "email", Some(true)), None);
    }

    #[test]
    fn end_to_end() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::create_dir_all(r.join("app/Livewire/Admin")).unwrap();
        std::fs::create_dir_all(r.join("resources/views/livewire/admin")).unwrap();
        std::fs::write(r.join("app/Livewire/Admin/UserProfile.php"), "<?php\nclass UserProfile\n{\n    public string $email = '';\n    public function save() {}\n}\n").unwrap();
        let view = r.join("resources/views/livewire/admin/user-profile.blade.php");
        let l = r#"<input wire:model="email"> <button wire:click="save">{{ $email }}"#;
        let h = help(r, &view, l, at(l, "email")).unwrap();
        assert!(h.contains("public string $email") && h.contains("UserProfile.php:4"), "{h}");
        let h = help(r, &view, l, at(l, "save")).unwrap();
        assert!(h.contains("method") && h.contains(":5"), "{h}");
        assert!(help(r, &r.join("resources/views/other.blade.php"), l, 20).is_none());
        assert!(help(r, &r.join("resources/views/livewire/missing.blade.php"), l, 20).is_none());
        assert_eq!(class_for_view(r, &view).unwrap().file_name().unwrap(), "UserProfile.php");
    }

    #[test]
    fn never_panics() {
        for t in ["é", "wire:", "wire:model=\"", "$", "wire:click=\"é", "{{ $é }}", "wire:model=\"日\" wire:"] {
            for c in 0..t.len() + 2 {
                let _ = target_at(t, c);
            }
        }
        let big = "wire:model=\"".repeat(100_000);
        let start = std::time::Instant::now();
        let _ = target_at(&big, big.len() - 3);
        assert!(start.elapsed().as_secs() < 5);
        let _ = find_member(&"public $a;\n".repeat(200_000), "zzz", None);
    }
}
