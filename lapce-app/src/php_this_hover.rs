//! Fallback hover for `$this->name` / `$this->name(...)` in a PHP file, used
//! only when the language server gives no answer (none installed, or it has
//! nothing). It finds the declaration of `name` in the same file:
//!
//! - a property (`public string $email`, `protected $x`, or a promoted
//!   constructor parameter), with any `#[Attribute]` lines directly above it
//!   (`#[Computed]`, `#[Url]`, `#[Locked]`...);
//! - a method (`public function save(...)`);
//! - for Eloquent models, where `name` appears in `$fillable`, `$guarded`,
//!   `$hidden`, `$visible`, `$appends`, `$dates` or `$casts` (with its cast).
//!
//! Inherited members and traits are not searched. Static text scan.

const MAX: usize = 1 << 20;
const MODEL_LISTS: &[&str] = &["fillable", "guarded", "hidden", "visible", "appends", "dates", "casts", "with"];

fn is_name(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The member name when `col` is on `$this->name` in `line`.
pub fn member_at(line: &str, col: usize) -> Option<&str> {
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
    let before = &line[..s];
    (s < e && (before.ends_with("$this->") || before.ends_with("$this?->"))).then(|| &line[s..e])
}

fn model_notes(text: &str, name: &str) -> Vec<String> {
    let mut notes = Vec::new();
    let mut list: Option<&str> = None;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if let Some(l) = MODEL_LISTS.iter().find(|l| {
            line.contains(&format!("${} ", l)) || line.contains(&format!("${}=", l))
        }) {
            if line.starts_with("protected") || line.starts_with("public") || line.starts_with("private") {
                list = Some(l);
            }
        }
        let Some(l) = list else { continue };
        for q in ['\'', '"'] {
            let key = format!("{q}{name}{q}");
            if let Some(p) = line.find(&key) {
                let rest = line[p + key.len()..].trim_start();
                if l == "casts" {
                    if let Some(v) = rest.strip_prefix("=>") {
                        let v = v.trim().trim_end_matches([',', ']', ';']).trim();
                        if !v.is_empty() && v.len() <= 80 {
                            notes.push(format!("`$casts`: {v} (line {})", i + 1));
                            continue;
                        }
                    }
                }
                notes.push(format!("in `${l}` (line {})", i + 1));
            }
        }
        if line.ends_with("];") || line.ends_with(");") {
            list = None;
        }
    }
    notes
}

/// Markdown for the hover on `$this->name`, from the file's own text.
pub fn help(text: &str, line: &str, col: usize) -> Option<String> {
    if text.len() > MAX {
        return None;
    }
    let name = member_at(line, col)?;
    let lines: Vec<&str> = text.lines().collect();
    let prop = format!("${name}");
    let mut found: Option<(usize, bool)> = None;
    for (i, raw) in lines.iter().enumerate() {
        let l = raw.trim();
        if let Some(p) = l.find("function ") {
            let rest = &l[p + 9..];
            let end = rest.find(|c: char| !is_name(c)).unwrap_or(rest.len());
            let before = &l[..p];
            let vis_ok = before.split_whitespace().all(|w| matches!(w, "public" | "protected" | "private" | "static" | "final" | "abstract"));
            if vis_ok && &rest[..end] == name {
                found = Some((i, true));
                break;
            }
            // Promoted constructor parameters, possibly on following lines.
            continue;
        }
        let declares = l.starts_with("public ") || l.starts_with("protected ") || l.starts_with("private ") || l.starts_with("var ")
            || l.starts_with("readonly ") || l.starts_with("final public");
        if declares {
            if let Some(p) = l.find(&prop) {
                if !l[p + prop.len()..].chars().next().is_some_and(is_name) {
                    found = Some((i, false));
                    break;
                }
            }
        }
    }
    let notes = model_notes(text, name);
    let (n, method) = match found {
        Some(f) => f,
        None if notes.is_empty() => return None,
        None => {
            return Some(format!("`$this->{name}`\n\n{}", notes.join("\n")));
        }
    };
    // Attribute lines directly above (`#[Computed]`).
    let mut start = n;
    while start > 0 && lines[start - 1].trim().starts_with("#[") {
        start -= 1;
    }
    let decl: Vec<&str> = lines[start..=n].iter().map(|l| l.trim()).collect();
    let kind = if method { "method" } else { "property" };
    let mut out = format!("**{kind}** `{name}`\n```php\n{}\n```\nline {}", decl.join("\n"), n + 1);
    if !notes.is_empty() {
        out.push_str("\n\n");
        out.push_str(&notes.join("\n"));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = "<?php\nclass User extends Model\n{\n    protected $fillable = ['name', 'email'];\n    protected $hidden = [\n        'password',\n    ];\n    protected $casts = [\n        'email_verified_at' => 'datetime',\n        'is_admin' => 'boolean',\n    ];\n\n    #[Url]\n    #[Locked]\n    public string $email = '';\n\n    public function __construct(private int $id) {}\n\n    public function save(): void\n    {\n        $this->email = 'x';\n    }\n}\n";

    fn at(l: &str, m: &str) -> usize {
        l.find(m).unwrap() + 1
    }

    #[test]
    fn member_detection() {
        let l = "$this->email = $this?->save() . $other->x;";
        assert_eq!(member_at(l, at(l, "email")), Some("email"));
        assert_eq!(member_at(l, at(l, "save")), Some("save"));
        assert_eq!(member_at(l, at(l, "other")), None);
        assert_eq!(member_at(l, at(l, "x;")), None);
        assert_eq!(member_at("$this->", 7), None);
        assert_eq!(member_at("é$this->é", 11), None);
    }

    #[test]
    fn property_with_attributes_and_method() {
        let l = "$this->email = 1; $this->save();";
        let h = help(MODEL, l, at(l, "email")).unwrap();
        assert!(h.contains("#[Url]") && h.contains("#[Locked]") && h.contains("public string $email"), "{h}");
        assert!(h.contains("line 15"), "{h}");
        let h = help(MODEL, l, at(l, "save")).unwrap();
        assert!(h.contains("**method**") && h.contains("public function save"), "{h}");
    }

    #[test]
    fn model_lists_and_casts() {
        let h = help(MODEL, "$this->is_admin", 9).unwrap();
        assert!(h.contains("`$casts`: 'boolean'"), "{h}");
        let h = help(MODEL, "$this->password", 9).unwrap();
        assert!(h.contains("in `$hidden`"), "{h}");
        let h = help(MODEL, "$this->name", 9).unwrap();
        assert!(h.contains("in `$fillable`"), "{h}");
        assert!(help(MODEL, "$this->nothing", 9).is_none());
    }

    #[test]
    fn never_panics() {
        for t in ["é", "$this->é", "public function", "#[", "protected $casts = ['a' =>", "function ("] {
            for c in 0..t.len() + 2 {
                let _ = help(t, t, c);
                let _ = help(MODEL, t, c);
            }
        }
        let big = "public $a;\n".repeat(90_000);
        let start = std::time::Instant::now();
        let _ = help(&big, "$this->zzz", 8);
        assert!(start.elapsed().as_secs() < 5);
        assert!(help(&"x".repeat(MAX + 1), "$this->a", 8).is_none());
    }
}
