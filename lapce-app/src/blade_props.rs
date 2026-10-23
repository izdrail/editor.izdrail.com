//! Go-to-definition from an attribute of a Blade component tag to where the
//! component declares it:
//!
//! ```blade
//! <x-forms.input type="email" :disabled="$off" />   {{-- caret on `disabled` --}}
//! ```
//!
//! opens `resources/views/components/forms/input.blade.php` on the line of
//! `'disabled'` inside its `@props([...])`, or, for a class component, the
//! constructor parameter `$disabled` in `app/View/Components/Forms/Input.php`.
//!
//! Static text scan only, nothing is executed. `wire:`, `x-`, `@`, `:class`
//! style framework attributes and attributes the component does not declare
//! resolve to nothing, so normal go-to carries on.
use std::path::{Path, PathBuf};

const MAX_FILE: u64 = 1 << 20;

fn is_attr_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.')
}

fn clamp(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// `foo-bar` -> `fooBar` (how Blade names the variable).
fn camel(attr: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in attr.chars() {
        if c == '-' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Component tag name and attribute name at byte `at` of `text`, where `text`
/// ends at (or after) the caret and starts a few lines above it.
pub fn tag_and_attr(text: &str, at: usize) -> Option<(String, String)> {
    let at = clamp(text, at);
    let open = text[..at].rfind("<x-")?;
    // Walk the tag from its start up to the caret, tracking quotes: the caret
    // must be outside quotes and the tag must not have closed.
    let mut quote: Option<char> = None;
    let mut brace = 0usize;
    for (i, c) in text[open..at].char_indices() {
        if i == 0 {
            continue;
        }
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '{' => brace += 1,
                '}' => brace = brace.saturating_sub(1),
                '>' if brace == 0 => return None,
                _ => {}
            },
        }
    }
    if quote.is_some() {
        return None;
    }
    let name_start = open + 3;
    let name_end = text[name_start..]
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .map(|n| name_start + n)?;
    if name_end == name_start || at <= name_end {
        return None; // caret on the tag name itself: template_refs handles it
    }
    // Attribute word around the caret.
    let mut s = at;
    while s > name_end {
        let p = text[..s].chars().next_back()?;
        if is_attr_char(p) { s -= p.len_utf8() } else { break }
    }
    let mut e = at;
    while let Some(c) = text[e..].chars().next() {
        if is_attr_char(c) { e += c.len_utf8() } else { break }
    }
    if s == e || s <= name_end {
        return None;
    }
    let word = &text[s..e];
    // Not an attribute name if what follows is not `=`, space, `/` or `>`.
    let word = word.strip_prefix("::").or_else(|| word.strip_prefix(':')).unwrap_or(word);
    if word.is_empty() || word.contains(':') || word.contains('.') || word.starts_with("x-") {
        return None;
    }
    Some((text[name_start..name_end].to_string(), word.to_string()))
}

fn read(path: &Path) -> Option<String> {
    let md = std::fs::symlink_metadata(path).ok()?;
    if !md.is_file() || md.len() > MAX_FILE {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// 0-based line of the prop `name` (camelCase) inside `@props(...)`.
pub fn prop_line(source: &str, name: &str) -> Option<usize> {
    let start = source.find("@props")?;
    let rest = &source[start..];
    // The directive ends at the matching `)`; scan at most 8 KB.
    let mut depth = 0i32;
    let mut end = rest.len().min(8192);
    let mut began = false;
    for (i, c) in rest.char_indices().take(8192) {
        match c {
            '(' => {
                depth += 1;
                began = true;
            }
            ')' => {
                depth -= 1;
                if began && depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &rest[..clamp(rest, end)];
    let base_line = source[..start].matches('\n').count();
    for (n, line) in body.lines().enumerate() {
        let mut from = 0;
        while let Some(p) = line[from..].find(['\'', '"']) {
            let q = line.as_bytes()[from + p] as char;
            let s = from + p + 1;
            let Some(len) = line[s..].find(q) else { break };
            let is_value = line[..from + p].trim_end().ends_with("=>");
            if !is_value && (&line[s..s + len] == name || camel(&line[s..s + len]) == name) {
                // A key (`'x' =>`) or a bare list item (`'x',` / `'x'` end).
                let after = line[s + len + 1..].trim_start();
                if after.starts_with("=>") || after.starts_with(',') || after.is_empty() || after.starts_with(']') {
                    return Some(base_line + n);
                }
            }
            from = s + len + 1;
        }
    }
    None
}

/// 0-based line of the constructor parameter `$name`.
pub fn ctor_line(source: &str, name: &str) -> Option<usize> {
    let ctor = source.find("function __construct")?;
    let needle = format!("${name}");
    let base_line = source[..ctor].matches('\n').count();
    let tail = &source[ctor..source.len().min(ctor + 4096)];
    for (n, line) in tail.lines().enumerate() {
        if let Some(p) = line.find(&needle) {
            let next = line[p + needle.len()..].chars().next();
            if !next.map_or(false, |c| c.is_ascii_alphanumeric() || c == '_') {
                return Some(base_line + n);
            }
        }
        if line.contains('{') && n > 0 && !line.contains('$') {
            break;
        }
    }
    None
}

/// `fooBar` -> `foo-bar` (how the attribute is typed in a tag).
pub(crate) fn kebab(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            if !out.is_empty() {
                out.push('-');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Prop names declared in a view component's `@props(...)`, in
/// declaration order, as typed in tags (camelCase keys become kebab).
pub fn prop_names(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(start) = source.find("@props") else { return out };
    let rest = &source[start..];
    // The directive ends at the matching `)`; scan at most 8 KB.
    let mut depth = 0i32;
    let mut end = rest.len().min(8192);
    let mut began = false;
    for (i, c) in rest.char_indices().take(8192) {
        match c {
            '(' => {
                depth += 1;
                began = true;
            }
            ')' => {
                depth -= 1;
                if began && depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &rest[..clamp(rest, end)];
    for line in body.lines() {
        let mut from = 0;
        while let Some(p) = line[from..].find(['\'', '"']) {
            let q = line.as_bytes()[from + p] as char;
            let s = from + p + 1;
            let Some(len) = line[s..].find(q) else { break };
            let name = &line[s..s + len];
            let is_value = line[..from + p].trim_end().ends_with("=>");
            let after = line[s + len + 1..].trim_start();
            let is_name = after.starts_with("=>")
                || after.starts_with(',')
                || after.is_empty()
                || after.starts_with(']');
            if !is_value && is_name && !name.is_empty() {
                let name = kebab(name);
                if !out.contains(&name) {
                    out.push(name);
                }
            }
            from = s + len + 1;
        }
    }
    out.truncate(60);
    out
}

/// Constructor parameter names of a class component, in declaration
/// order, as typed in tags.
pub fn ctor_param_names(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(ctor) = source.find("function __construct") else { return out };
    let tail = &source[ctor..source.len().min(ctor + 4096)];
    let Some(open) = tail.find('(') else { return out };
    let mut depth = 0i32;
    let mut end = tail.len();
    for (i, c) in tail[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let params = &tail[open + 1..end];
    let bytes = params.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let s = i + 1;
            let mut e = s;
            while e < bytes.len() && (bytes[e].is_ascii_alphanumeric() || bytes[e] == b'_') {
                e += 1;
            }
            if e > s {
                let name = kebab(&params[s..e]);
                if name != "this" && !out.contains(&name) {
                    out.push(name);
                }
            }
            i = e;
        } else {
            i += 1;
        }
    }
    out.truncate(60);
    out
}

/// Prop names of the component named `tag` (`forms.input`), in
/// declaration order: `@props` names of a view component, constructor
/// parameters of a class component.
pub fn component_prop_names(root: &Path, tag: &str) -> Vec<String> {
    let Some(target) = crate::template_refs::blade_component(root, tag) else {
        return Vec::new();
    };
    let Some(source) = read(&target) else { return Vec::new() };
    if target.extension().and_then(|e| e.to_str()) == Some("php")
        && !target.to_string_lossy().ends_with(".blade.php")
    {
        ctor_param_names(&source)
    } else {
        prop_names(&source)
    }
}

/// Target file and 0-based line for the attribute at `at` of `text`.
pub fn resolve(root: &Path, file: &Path, text: &str, at: usize) -> Option<(PathBuf, usize)> {
    if !file.file_name()?.to_str()?.ends_with(".blade.php") {
        return None;
    }
    let (tag, attr) = tag_and_attr(text, at)?;
    let target = crate::template_refs::blade_component(root, &tag)?;
    let source = read(&target)?;
    let name = camel(&attr);
    let line = if target.extension().and_then(|e| e.to_str()) == Some("php")
        && !target.to_string_lossy().ends_with(".blade.php")
    {
        ctor_line(&source, &name)?
    } else {
        prop_line(&source, &name)?
    };
    Some((target, line))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str, marker: &str) -> usize {
        text.find(marker).unwrap() + 1
    }

    #[test]
    fn finds_tag_and_attribute() {
        let t = r#"<x-forms.input type="email" :disabled="$off" />"#;
        assert_eq!(tag_and_attr(t, at(t, "type")), Some(("forms.input".into(), "type".into())));
        assert_eq!(tag_and_attr(t, at(t, "disabled")), Some(("forms.input".into(), "disabled".into())));
    }

    #[test]
    fn multi_line_tag() {
        let t = "<x-alert\n    type=\"error\"\n    dismiss-after=\"5\"\n>";
        assert_eq!(tag_and_attr(t, at(t, "dismiss")), Some(("alert".into(), "dismiss-after".into())));
    }

    #[test]
    fn not_on_name_value_or_after_close() {
        let t = r#"<x-card title="Hello world" />"#;
        assert_eq!(tag_and_attr(t, at(t, "card")), None);
        assert_eq!(tag_and_attr(t, at(t, "Hello")), None);
        let t = r#"<x-card title="a" /> <p class="x">"#;
        assert_eq!(tag_and_attr(t, at(t, "class")), None);
        let t = r#"<x-card :items="$a->b" label="z">"#;
        assert_eq!(tag_and_attr(t, at(t, "label")), Some(("card".into(), "label".into())));
    }

    #[test]
    fn framework_attributes_are_skipped() {
        let t = r#"<x-card wire:model="a" x-on:click="b" @click="c" x-show="d">"#;
        assert_eq!(tag_and_attr(t, at(t, "wire")), None);
        assert_eq!(tag_and_attr(t, at(t, "x-on")), None);
        assert_eq!(tag_and_attr(t, at(t, "x-show")), None);
    }

    #[test]
    fn props_lines() {
        let src = "@props([\n    'type' => 'text',\n    'disabled' => false,\n    'dismissAfter' => 5,\n])\n<input>";
        assert_eq!(prop_line(src, "type"), Some(1));
        assert_eq!(prop_line(src, "disabled"), Some(2));
        assert_eq!(prop_line(src, "dismissAfter"), Some(3));
        assert_eq!(prop_line(src, "missing"), None);
        assert_eq!(prop_line("<div>@props(['a', 'b'])</div>", "b"), Some(0));
        assert_eq!(prop_line("@props(['a' => 'b'])", "b"), None);
        assert_eq!(prop_line("no props here 'x'", "x"), None);
    }

    #[test]
    fn ctor_params() {
        let src = "<?php\nclass Alert {\n    public function __construct(\n        public string $type,\n        public int $dismissAfter = 5,\n    ) {}\n}";
        assert_eq!(ctor_line(src, "type"), Some(3));
        assert_eq!(ctor_line(src, "dismissAfter"), Some(4));
        assert_eq!(ctor_line(src, "dismiss"), None);
    }

    #[test]
    fn resolves_end_to_end() {
        let d = tempfile::tempdir().unwrap();
        let v = d.path().join("resources/views/components/forms");
        std::fs::create_dir_all(&v).unwrap();
        std::fs::write(v.join("input.blade.php"), "@props([\n 'type' => 'text',\n 'maxLength',\n])\n").unwrap();
        let c = d.path().join("app/View/Components");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(c.join("Alert.php"), "<?php\nclass Alert {\n public function __construct(public string $kind) {}\n}").unwrap();
        let f = d.path().join("resources/views/p.blade.php");
        let t = r#"<x-forms.input type="x" max-length="3" other="1" />"#;
        let r = resolve(d.path(), &f, t, at(t, "type")).unwrap();
        assert!(r.0.ends_with("forms/input.blade.php"));
        assert_eq!(r.1, 1);
        assert_eq!(resolve(d.path(), &f, t, at(t, "max-length")).unwrap().1, 2);
        assert!(resolve(d.path(), &f, t, at(t, "other")).is_none());
        let t = r#"<x-alert kind="a" />"#;
        let r = resolve(d.path(), &f, t, at(t, "kind")).unwrap();
        assert!(r.0.ends_with("Alert.php"));
        assert_eq!(r.1, 2);
        assert!(resolve(d.path(), Path::new("a.php"), t, 9).is_none());
    }

    #[test]
    fn prop_names_keys_and_list_items() {
        let src = "@props([\n    'type' => 'text',\n    'label',\n    'maxLength' => 20,\n])\n<div>{{ $label }}</div>\n";
        assert_eq!(prop_names(src), ["type", "label", "max-length"]);
        // No directive: nothing.
        assert!(prop_names("<div>x</div>").is_empty());
        // Default values are not names.
        let src2 = "@props(['tone' => 'info'])";
        assert_eq!(prop_names(src2), ["tone"]);
    }

    #[test]
    fn ctor_params_in_order() {
        let src = "class Alert {\n    public function __construct(\n        public string $type,\n        public bool $dismissible = false,\n        protected $maxLength = 20,\n    ) {}\n}\n";
        assert_eq!(ctor_param_names(src), ["type", "dismissible", "max-length"]);
        assert!(ctor_param_names("class A {}").is_empty());
    }

    #[test]
    fn never_panics() {
        let t = "<x-é ü=\"é\" <x-";
        for i in 0..t.len() + 3 {
            let _ = tag_and_attr(t, i);
        }
        let _ = prop_line("@props(", "a");
        let _ = prop_line("@props([\"é", "a");
        let _ = ctor_line("function __construct(", "a");
    }
}
