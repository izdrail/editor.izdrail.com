//! Go to a template or component referenced under the cursor, without a
//! language server: Blade (`@include('a.b')`, `@extends`, `view('a.b')`,
//! `<x-forms.input>`, `<livewire:user-profile>`, `@livewire('user-profile')`) and Twig (`{% extends 'base.html.twig' %}`, `include`,
//! `embed`, `<twig:Alert>`, `component('Alert')`).
//!
//! Purely textual: it reads the cursor's line and checks which files exist
//! under the workspace root. Nothing is executed and nothing outside the
//! workspace is looked at. `None` means "not a template reference", and the
//! caller falls back to the normal definition lookup.
use std::path::{Path, PathBuf};

const MAX_NAME: usize = 200;

/// Calls whose first string argument names a view/template. The text before
/// the quote must end with one of these (ignoring spaces and `(`/`,`).
const VIEW_CALLERS: &[&str] = &[
    "@include", "@includeIf", "@includeWhen", "@includeUnless", "@includeFirst",
    "@extends", "@each", "@component", "@componentFirst", "view", "View::make",
    "View::first", "markdown", "renderView", "render", "Route::view",
];
const TWIG_CALLERS: &[&str] = &[
    "extends", "include", "embed", "import", "from", "use", "source", "include(",
    "render", "template_from_string",
];

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/' | '@' | ':')
}

/// The quoted string containing byte `col`: `(contents, text_before_quote)`.
fn string_at(line: &str, col: usize) -> Option<(&str, &str)> {
    let col = col.min(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\'' || bytes[i] == b'"' {
            let q = bytes[i];
            let start = i + 1;
            let end = start + line[start..].find(q as char)?;
            if col >= start && col <= end {
                return Some((&line[start..end], &line[..i]));
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    None
}

/// True when the string starting right after `before` is the template-name
/// argument of one of `callers`: the nearest caller on the line is directly
/// followed by `(` (or `{%`-tag keyword) and nothing but whitespace, or, for
/// the condition-first forms, a quote-free first argument and a comma.
fn called_by(before: &str, callers: &[&str]) -> bool {
    if before.contains("{%") {
        // Twig tag: `{% extends 'x' %}`, `{% include "x" with {...} %}`.
        let tag = before[before.rfind("{%").unwrap() + 2..].trim();
        return callers.iter().any(|c| tag == c.trim_end_matches('('));
    }
    callers.iter().any(|c| {
        let c = c.trim_end_matches('(');
        before.rfind(c).is_some_and(|i| {
            if i > 0 {
                let prev = before[..i].chars().next_back().unwrap();
                if prev.is_ascii_alphanumeric() || prev == '_' || prev == '$' {
                    return false;
                }
            }
            let rest = before[i + c.len()..].trim_start();
            let Some(args) = rest.strip_prefix('(') else {
                // Directive without parentheses is not a call.
                return false;
            };
            let args = args.trim();
            if args.is_empty() {
                return true;
            }
            (c == "@includeWhen" || c == "@includeUnless")
                && args.ends_with(',')
                && !args.contains(|ch| ch == '\'' || ch == '"')
        })
    })
}

fn safe(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && !name.contains("..")
        && !name.starts_with('/')
        && !name.contains("::")
        && name.chars().all(is_name_char)
}

fn first_existing(cands: Vec<PathBuf>) -> Option<PathBuf> {
    cands.into_iter().find(|p| p.is_file())
}

fn blade_view(root: &Path, name: &str) -> Option<PathBuf> {
    if !safe(name) {
        return None;
    }
    let rel = name.replace('.', "/");
    let base = root.join("resources").join("views");
    first_existing(vec![
        base.join(format!("{rel}.blade.php")),
        base.join(format!("{rel}.php")),
        base.join(rel).join("index.blade.php"),
    ])
}

fn twig_template(root: &Path, name: &str) -> Option<PathBuf> {
    if !safe(name) || name.starts_with('@') {
        return None;
    }
    first_existing(vec![root.join("templates").join(name)])
}

fn studly(part: &str) -> String {
    part.split(|c| c == '-' || c == '_')
        .map(|w| {
            let mut cs = w.chars();
            cs.next()
                .map(|f| f.to_ascii_uppercase().to_string() + cs.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// `<x-forms.input` -> view `components/forms/input` or class `Forms/Input.php`.
fn blade_component(root: &Path, name: &str) -> Option<PathBuf> {
    if !safe(name) || name.contains('/') || name.contains(':') {
        return None;
    }
    let rel = name.replace('.', "/");
    let views = root.join("resources").join("views").join("components");
    let class: PathBuf = name.split('.').map(studly).collect::<Vec<_>>().join("/").into();
    first_existing(vec![
        views.join(format!("{rel}.blade.php")),
        views.join(&rel).join("index.blade.php"),
        root.join("app").join("View").join("Components").join(class).with_extension("php"),
    ])
}


/// `<livewire:admin.user-profile` / `@livewire('admin.user-profile')` ->
/// `app/Livewire/Admin/UserProfile.php` (v3), `app/Http/Livewire/...` (v2),
/// else `resources/views/livewire/admin/user-profile.blade.php`.
fn livewire_component(root: &Path, name: &str) -> Option<PathBuf> {
    if !safe(name) || name.contains('/') || name.contains(':') {
        return None;
    }
    let rel = name.replace('.', "/");
    let class: PathBuf = name.split('.').map(studly).collect::<Vec<_>>().join("/").into();
    first_existing(vec![
        root.join("app").join("Livewire").join(&class).with_extension("php"),
        root.join("app").join("Http").join("Livewire").join(&class).with_extension("php"),
        root.join("resources").join("views").join("livewire").join(format!("{rel}.blade.php")),
    ])
}

/// `<twig:Alert` / `<twig:Form:Input` -> `templates/components/Form/Input.html.twig`.
fn twig_component(root: &Path, name: &str) -> Option<PathBuf> {
    if !safe(name) || name.contains('/') || name.contains('.') || name.contains('@') {
        return None;
    }
    let rel = name.replace(':', "/");
    let base = root.join("templates").join("components");
    first_existing(vec![base.join(format!("{rel}.html.twig"))])
}

/// The tag name when `col` is inside `<prefix...` (up to the first non-name char).
fn tag_name_at<'a>(line: &'a str, col: usize, prefix: &str) -> Option<&'a str> {
    let col = col.min(line.len());
    let open = format!("<{prefix}");
    let start = line[..col.max(open.len().min(line.len()))]
        .rfind(&open)
        .or_else(|| line[..].get(..col).and_then(|s| s.rfind(&open)))?;
    let name_start = start + open.len();
    let name_end = line[name_start..]
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':')))
        .map(|n| name_start + n)
        .unwrap_or(line.len());
    (col >= start && col <= name_end && name_end > name_start).then(|| &line[name_start..name_end])
}

/// Template file referenced at byte `col` of `line`, for the file `file_name`.
pub fn resolve(root: &Path, file: &Path, line: &str, col: usize) -> Option<PathBuf> {
    let name = file.file_name()?.to_str()?;
    let blade = name.ends_with(".blade.php");
    let twig = name.ends_with(".twig");
    let php = name.ends_with(".php");
    if !(blade || twig || php) {
        return None;
    }
    if blade {
        if let Some(tag) = tag_name_at(line, col, "x-") {
            return blade_component(root, tag);
        }
    }
    if blade || php {
        if let Some(tag) = tag_name_at(line, col, "livewire:") {
            return livewire_component(root, tag);
        }
    }
    if twig {
        if let Some(tag) = tag_name_at(line, col, "twig:") {
            return twig_component(root, tag);
        }
    }
    let (s, before) = string_at(line, col)?;
    if twig {
        if called_by(before, &["component"]) {
            return twig_component(root, s);
        }
        if called_by(before, TWIG_CALLERS) {
            return twig_template(root, s);
        }
        return None;
    }
    if (blade || php) && called_by(before, &["@livewire"]) {
        return livewire_component(root, s);
    }
    if called_by(before, VIEW_CALLERS) {
        return blade_view(root, s);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(root: &Path, rel: &str) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "x").unwrap();
        p
    }

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    #[test]
    fn blade_directives_and_view_calls() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let nav = touch(r, "resources/views/partials/nav.blade.php");
        let app = touch(r, "resources/views/layouts/app.blade.php");
        let idx = touch(r, "resources/views/users/index.blade.php");
        let f = r.join("resources/views/home.blade.php");
        let l = "@include('partials.nav', ['a' => 1])";
        assert_eq!(resolve(r, &f, l, at(l, "nav")), Some(nav));
        let l = "@extends(\"layouts.app\")";
        assert_eq!(resolve(r, &f, l, at(l, "app")), Some(app));
        let ctl = r.join("app/Http/Controllers/UserController.php");
        let l = "        return view('users.index', compact('users'));";
        assert_eq!(resolve(r, &ctl, l, at(l, "index")), Some(idx));
        // The second string is not a view name.
        assert_eq!(resolve(r, &ctl, l, at(l, "users')")), None);
        // Missing view, or an arbitrary string: not a reference.
        let l = "@include('missing.view')";
        assert_eq!(resolve(r, &f, l, at(l, "missing")), None);
        let l = "$x = route('users.index');";
        assert_eq!(resolve(r, &ctl, l, at(l, "users")), None);
    }

    #[test]
    fn include_when_takes_the_view_in_the_second_argument() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let p = touch(r, "resources/views/a/b.blade.php");
        let f = r.join("x.blade.php");
        let l = "@includeWhen($ok, 'a.b')";
        assert_eq!(resolve(r, &f, l, at(l, "a.b")), Some(p));
    }

    #[test]
    fn blade_components_prefer_view_then_class() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let alert = touch(r, "resources/views/components/alert.blade.php");
        let input = touch(r, "resources/views/components/forms/input/index.blade.php");
        let class = touch(r, "app/View/Components/UserCard.php");
        let f = r.join("resources/views/home.blade.php");
        let l = "<x-alert type=\"error\" />";
        assert_eq!(resolve(r, &f, l, at(l, "alert")), Some(alert));
        let l = "  <x-forms.input name=\"a\">";
        assert_eq!(resolve(r, &f, l, at(l, "input")), Some(input));
        let l = "<x-user-card :user=\"$u\"/>";
        assert_eq!(resolve(r, &f, l, at(l, "card")), Some(class));
        // Cursor on an attribute, not the tag name.
        let l = "<x-alert type=\"error\" />";
        assert_eq!(resolve(r, &f, l, at(l, "error")), None);
    }

    #[test]
    fn twig_templates_and_components() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let base = touch(r, "templates/base.html.twig");
        let inc = touch(r, "templates/partials/_nav.html.twig");
        let comp = touch(r, "templates/components/Alert.html.twig");
        let nested = touch(r, "templates/components/Form/Input.html.twig");
        let f = r.join("templates/home.html.twig");
        let l = "{% extends 'base.html.twig' %}";
        assert_eq!(resolve(r, &f, l, at(l, "base")), Some(base));
        let l = "{% include \"partials/_nav.html.twig\" with {a: 1} %}";
        assert_eq!(resolve(r, &f, l, at(l, "_nav")), Some(inc.clone()));
        let l = "{{ include('partials/_nav.html.twig') }}";
        assert_eq!(resolve(r, &f, l, at(l, "_nav")), Some(inc));
        let l = "<twig:Alert type=\"x\" />";
        assert_eq!(resolve(r, &f, l, at(l, "Alert")), Some(comp.clone()));
        let l = "<twig:Form:Input name=\"a\">";
        assert_eq!(resolve(r, &f, l, at(l, "Input")), Some(nested));
        let l = "{{ component('Alert', {a: 1}) }}";
        assert_eq!(resolve(r, &f, l, at(l, "Alert")), Some(comp));
        // Namespaced (@bundle) templates are not guessed.
        let l = "{% extends '@Acme/base.html.twig' %}";
        assert_eq!(resolve(r, &f, l, at(l, "base")), None);
    }

    #[test]
    fn escapes_and_other_files_are_rejected() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        touch(r, "secret.blade.php");
        let f = r.join("resources/views/home.blade.php");
        for l in ["@include('../../secret')", "@include('/etc/passwd')", "@include('a::b')"] {
            assert_eq!(resolve(r, &f, l, at(l, "'") + 2), None);
        }
        assert_eq!(resolve(r, Path::new("a.rs"), "view('x')", 7), None);
        assert_eq!(resolve(r, &f, "", 0), None);
    }

    #[test]
    fn livewire_components() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let v3 = touch(r, "app/Livewire/Admin/UserProfile.php");
        let v2 = touch(r, "app/Http/Livewire/Counter.php");
        let view = touch(r, "resources/views/livewire/only-view.blade.php");
        let f = r.join("resources/views/home.blade.php");
        let l = "<livewire:admin.user-profile :user=\"$u\" />";
        assert_eq!(resolve(r, &f, l, at(l, "user-profile")), Some(v3.clone()));
        let l = "@livewire('admin.user-profile')";
        assert_eq!(resolve(r, &f, l, at(l, "admin")), Some(v3));
        let l = "<livewire:counter />";
        assert_eq!(resolve(r, &f, l, at(l, "counter")), Some(v2));
        let l = "<livewire:only-view/>";
        assert_eq!(resolve(r, &f, l, at(l, "only")), Some(view));
        // Attribute value and unknown components are not references.
        let l = "<livewire:counter wire:key=\"x\" />";
        assert_eq!(resolve(r, &f, l, at(l, "wire:key")), None);
        let l = "<livewire:missing />";
        assert_eq!(resolve(r, &f, l, at(l, "missing")), None);
    }
}
