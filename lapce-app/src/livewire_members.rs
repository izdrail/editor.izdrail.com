//! "Livewire Members" palette: the public properties and methods of the
//! Livewire component open in the editor. Works from the class
//! (`app/Livewire/**`, `app/Http/Livewire/**`) or from its view
//! (`resources/views/livewire/**.blade.php`, mapped to the class by name).
//!
//! Static text scan, nothing is executed. Bounded: 1 MB class file, 300
//! members, symlinks not followed.
use std::path::{Path, PathBuf};

use crate::route_index::RouteTarget;

const MAX_FILE: u64 = 1 << 20;
const MAX_MEMBERS: usize = 300;

#[derive(Debug, PartialEq, Eq)]
pub struct Member {
    /// `property`, `action` (public method) or `hook`.
    pub kind: &'static str,
    pub name: String,
    /// Attribute names on the line(s) above, e.g. `Computed`, `Locked`.
    pub attrs: Vec<String>,
    pub line: usize,
}

/// `user-profile` / `user_profile` -> `UserProfile`.
fn studly(seg: &str) -> String {
    seg.split(|c| c == '-' || c == '_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect()
}

/// Class file for a Livewire class or view path, if one exists.
pub fn class_file(root: &Path, file: &Path) -> Option<PathBuf> {
    for base in ["app/Livewire", "app/Http/Livewire"] {
        if file.strip_prefix(root.join(base)).is_ok() && file.extension().is_some_and(|e| e == "php") {
            return Some(file.to_path_buf());
        }
    }
    let rel = file.strip_prefix(root.join("resources/views/livewire")).ok()?;
    let rel = rel.to_str()?.strip_suffix(".blade.php")?;
    let mapped: Vec<String> = rel.split('/').map(studly).collect();
    if mapped.iter().any(|s| s.is_empty()) {
        return None;
    }
    let tail = format!("{}.php", mapped.join("/"));
    ["app/Livewire", "app/Http/Livewire"]
        .iter()
        .map(|b| root.join(b).join(&tail))
        .find(|p| std::fs::symlink_metadata(p).map_or(false, |m| m.is_file()))
}

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
            None if c == '\'' || c == '"' => q = Some(c),
            None => out.push(c),
        }
    }
    out
}

fn ident(s: &str) -> String {
    s.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect()
}

fn attr_names(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(p) = rest.find("#[") {
        rest = &rest[p + 2..];
        let n = ident(rest.trim_start());
        if !n.is_empty() {
            out.push(n);
        }
    }
    out
}

/// Public members declared directly in the first class of `text`.
pub fn parse(text: &str) -> Vec<Member> {
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut in_class = false;
    let mut pending: Vec<String> = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let code = strip_strings(line);
        if !in_class {
            let t = code.trim_start();
            let t = t.strip_prefix("final ").or_else(|| t.strip_prefix("abstract ")).unwrap_or(t);
            if t.starts_with("class ") {
                in_class = true;
            } else {
                continue;
            }
        } else if depth == 1 {
            if line.starts_with("#[") {
                pending.extend(attr_names(line));
            }
            if let Some(rest) = line.strip_prefix("public ") {
                let rest = rest.trim_start();
                let rest = rest.strip_prefix("static ").unwrap_or(rest);
                if let Some(p) = rest.find("function ") {
                    let name = ident(rest[p + 9..].trim_start());
                    if !name.is_empty() && !name.starts_with("__") {
                        let kind = if name.starts_with("updat") && name.len() > 5 || name == "mount" || name == "boot" || name == "hydrate" || name == "dehydrate" { "hook" } else { "action" };
                        out.push(Member { kind, name, attrs: std::mem::take(&mut pending), line: i });
                    }
                } else if let Some(p) = rest.find('$') {
                    let name = ident(&rest[p + 1..]);
                    let before = &rest[..p];
                    if !name.is_empty() && !before.contains('(') {
                        out.push(Member { kind: "property", name, attrs: std::mem::take(&mut pending), line: i });
                    }
                }
            }
            if !line.starts_with("#[") && !line.is_empty() && !line.starts_with("//") && !line.starts_with('*') && !line.starts_with("/*") {
                pending.clear();
            }
            if out.len() >= MAX_MEMBERS {
                break;
            }
        }
        for c in code.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if in_class && depth <= 0 && code.contains('}') {
            break;
        }
    }
    out
}

pub fn discover(root: &Path, file: &Path) -> Vec<RouteTarget> {
    let Some(class) = class_file(root, file) else { return Vec::new() };
    if std::fs::symlink_metadata(&class).map_or(true, |m| !m.is_file() || m.len() > MAX_FILE) {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(&class) else { return Vec::new() };
    let shown = class.strip_prefix(root).unwrap_or(&class).display().to_string();
    parse(&text)
        .into_iter()
        .map(|m| {
            let attrs = if m.attrs.is_empty() { String::new() } else { format!(" #{}", m.attrs.join(" #")) };
            let label = match m.kind {
                "property" => format!("property ${}{attrs}  {shown}:{}", m.name, m.line + 1),
                k => format!("{k} {}(){attrs}  {shown}:{}", m.name, m.line + 1),
            };
            RouteTarget { label, path: class.clone(), line: m.line }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?php
namespace App\Livewire;

class UserProfile extends Component
{
    public string $name = 'a {';
    public $email;
    protected $rules = [];
    private int $hidden = 0;

    #[Locked]
    public int $id = 0;

    public function mount($id)
    {
        $x = ['public' => 1];
        if ($x) {
            $this->id = $id;
        }
    }

    #[Computed]
    public function posts() { return []; }

    public function save(): void {}
    public function updatedName($v) {}
    public function __construct() {}
    protected function helper() {}

    public function render()
    {
        return view('livewire.user-profile');
    }
}
"#;

    #[test]
    fn parses_public_members_only() {
        let m = parse(SAMPLE);
        let got: Vec<_> = m.iter().map(|m| (m.kind, m.name.as_str(), m.attrs.join(","))).collect();
        assert_eq!(
            got,
            [
                ("property", "name", String::new()),
                ("property", "email", String::new()),
                ("property", "id", "Locked".to_string()),
                ("hook", "mount", String::new()),
                ("action", "posts", "Computed".to_string()),
                ("action", "save", String::new()),
                ("hook", "updatedName", String::new()),
                ("action", "render", String::new()),
            ]
        );
        assert_eq!(m[0].line, 5);
    }

    #[test]
    fn junk_and_no_class() {
        assert!(parse("").is_empty());
        assert!(parse("<?php\npublic function x() {}").is_empty());
        assert!(parse("class A {\n public function é日(").len() <= 1);
    }

    #[test]
    fn maps_view_to_class_and_discovers() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let put = |rel: &str, body: &str| {
            let p = r.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        };
        put("app/Livewire/Admin/UserProfile.php", SAMPLE);
        put("resources/views/livewire/admin/user-profile.blade.php", "<div></div>");
        put("resources/views/livewire/lonely.blade.php", "<div></div>");
        put("app/Models/User.php", "class User {}");
        let class = r.join("app/Livewire/Admin/UserProfile.php");
        assert_eq!(studly("user_profile-x"), "UserProfileX");
        assert_eq!(class_file(r, &r.join("resources/views/livewire/admin/user-profile.blade.php")), Some(class.clone()));
        assert_eq!(class_file(r, &class), Some(class.clone()));
        assert_eq!(class_file(r, &r.join("resources/views/livewire/lonely.blade.php")), None);
        assert_eq!(class_file(r, &r.join("app/Models/User.php")), None);
        let from_view = discover(r, &r.join("resources/views/livewire/admin/user-profile.blade.php"));
        let from_class = discover(r, &class);
        assert_eq!(from_view.len(), 8);
        assert_eq!(from_view.len(), from_class.len());
        assert!(from_view[2].label.starts_with("property $id #Locked  app/Livewire/Admin/UserProfile.php:12"));
        assert!(discover(r, &r.join("none.php")).is_empty());
    }

    #[test]
    fn traversal_and_big_files() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::create_dir_all(r.join("app/Livewire")).unwrap();
        std::fs::write(r.join("app/Livewire/Big.php"), format!("class Big {{\n{}}}", "public $a;\n".repeat(200_000))).unwrap();
        assert!(discover(r, &r.join("app/Livewire/Big.php")).is_empty());
        let many = format!("class M {{\n{}}}", "public $a;\n".repeat(1000));
        assert_eq!(parse(&many).len(), MAX_MEMBERS);
        assert_eq!(class_file(r, &r.join("resources/views/livewire/../x.blade.php")), None);
    }

    #[test]
    fn random_text_never_panics() {
        let pieces = ["class A {", "public ", "function ", "$p", "#[Locked]", "{", "}", "(", ")", "'", "\"", "é", "日", "\n", " ", "static "];
        let mut s = 17u64;
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
