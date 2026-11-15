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
/// Symfony controller and service calls whose first argument is a Twig template name.
const SYMFONY_RENDER: &[&str] = &[
    "render", "renderView", "renderForm", "renderBlock", "renderBlockView", "stream", "load",
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
            let mut end = start;
            while end < bytes.len() {
                if bytes[end] == b'\\' {
                    end = (end + 2).min(bytes.len());
                } else if bytes[end] == q {
                    break;
                } else {
                    end += 1;
                }
            }
            if end == bytes.len() {
                return None;
            }
            if col + 1 >= start && col <= end {
                return Some((&line[start..end], &line[..i]));
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    None
}

/// Index of the argument about to start. Quoted commas and nested
/// expressions belong to the preceding argument, not the outer call.
fn pending_argument(args: &str) -> Option<usize> {
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut index = 0;
    let mut tail = 0;
    for (i, ch) in args.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => stack.push(ch),
            ')' | ']' | '}' => {
                let expected = match ch { ')' => '(', ']' => '[', _ => '{' };
                if stack.pop() != Some(expected) {
                    return None;
                }
            }
            ',' if stack.is_empty() => {
                if args[tail..i].trim().is_empty() {
                    return None;
                }
                index += 1;
                tail = i + 1;
            }
            _ => {}
        }
    }
    (quote.is_none() && stack.is_empty() && args[tail..].trim().is_empty())
        .then_some(index)
}

/// Does the next string occupy this caller's template-name argument?
fn called_by(before: &str, callers: &[&str]) -> bool {
    if before.contains("{%") {
        let tag = before[before.rfind("{%").unwrap() + 2..].trim();
        return callers.iter().any(|c| tag == c.trim_end_matches('('));
    }
    callers.iter().any(|c| {
        let c = c.trim_end_matches('(');
        before.rfind(c).is_some_and(|i| {
            if i > 0 {
                let prev = before[..i].chars().next_back().unwrap();
                if prev.is_ascii_alphanumeric() || matches!(prev, '_' | '$' | ':') {
                    return false;
                }
            }
            let rest = before[i + c.len()..].trim_start();
            let Some(args) = rest.strip_prefix('(') else {
                return false;
            };
            let expected = if matches!(c, "Route::view" | "@includeWhen" | "@includeUnless") {
                1
            } else {
                0
            };
            pending_argument(args) == Some(expected)
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
pub(crate) fn blade_component(root: &Path, name: &str) -> Option<PathBuf> {
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
pub(crate) fn livewire_component(root: &Path, name: &str) -> Option<PathBuf> {
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
    let col = clamp_col(line, col);
    let open = format!("<{prefix}");
    // Last opening tag that starts at or before the cursor.
    let upto = clamp_col(line, col.saturating_add(open.len()));
    let start = line[..upto].rfind(&open)?;
    let name_start = start + open.len();
    let name_end = line[name_start..]
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':')))
        .map(|n| name_start + n)
        .unwrap_or(line.len());
    (col >= start && col <= name_end && name_end > name_start).then(|| &line[name_start..name_end])
}

/// `col` clamped to the line and moved back onto a char boundary, so slicing
/// can never panic on multibyte text.
fn clamp_col(line: &str, col: usize) -> usize {
    let mut c = col.min(line.len());
    while !line.is_char_boundary(c) {
        c -= 1;
    }
    c
}

/// Template file referenced at byte `col` of `line`, for the file `file_name`.
pub fn resolve(root: &Path, file: &Path, line: &str, col: usize) -> Option<PathBuf> {
    let col = clamp_col(line, col);
    let name = file.file_name()?.to_str()?;
    let blade = crate::builtin_snippets::snippet_language(file, "php") == "blade";
    let twig = name.to_ascii_lowercase().ends_with(".twig");
    let php = matches!(
        lapce_core::language::LapceLanguage::from_path_raw(file),
        Some(lapce_core::language::LapceLanguage::Php
            | lapce_core::language::LapceLanguage::Blade)
    );
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
    if php && s.ends_with(".twig") {
        // Symfony controllers: `$this->render('blog/index.html.twig')`.
        if called_by(before, SYMFONY_RENDER) {
            return twig_template(root, s);
        }
        return None;
    }
    if called_by(before, VIEW_CALLERS) {
        return blade_view(root, s);
    }
    None
}

/// Markdown hover for the template or component reference at `col`: the
/// file it resolves to. Mirrors `resolve`, so the same references are
/// covered (Blade `view()`, `@include`, `x-` tags, Twig `{% extends %}` /
/// `{% include %}` / `twig:` tags, Symfony `render('x.html.twig')`).
pub fn hover(root: &Path, file: &Path, line: &str, col: usize) -> Option<String> {
    let target = resolve(root, file, line, col)?;
    let shown = target.strip_prefix(root).unwrap_or(&target).display();
    Some(format!("**Template** `{shown}`\n\n_Ctrl+click opens the file._"))
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
    fn hover_shows_the_resolved_template() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        touch(r, "resources/views/users/index.blade.php");
        touch(r, "templates/base.html.twig");

        let l = "return view('users.index');";
        let f = Path::new("app/Http/Controllers/UserController.php");
        let md = hover(r, f, l, at(l, "users")).unwrap();
        assert!(md.contains("resources/views/users/index.blade.php"), "{md}");
        // The opening quote counts as inside the string.
        let open = l.find("'users.index'").unwrap();
        assert_eq!(hover(r, f, l, open), hover(r, f, l, open + 2));

        let l = "{% extends 'base.html.twig' %}";
        let f = Path::new("templates/page.html.twig");
        let md = hover(r, f, l, at(l, "base")).unwrap();
        assert!(md.contains("templates/base.html.twig"), "{md}");

        touch(r, "templates/components/Alert.html.twig");
        let l = "<twig:Alert />";
        let md = hover(r, f, l, at(l, "Alert")).unwrap();
        assert!(md.contains("templates/components/Alert.html.twig"), "{md}");
        let l = "plain text 'base.html.twig'";
        assert!(hover(r, f, l, at(l, "base")).is_none());

        // Unknown template: no hover.
        let l = "return view('missing.index');";
        let f = Path::new("controller.php");
        assert!(hover(r, f, l, at(l, "missing")).is_none());
    }

    #[test]
    fn php_cli_templates_use_the_same_path_routing_as_syntax() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let view = touch(r, "resources/views/reports/index.blade.php");
        let twig = touch(r, "templates/reports/index.html.twig");
        let line = "view('reports.index')";
        let render = "$twig->render('reports/index.html.twig')";
        for name in ["artisan", "project/artisan", "bin/console", "project/bin/console"] {
            assert_eq!(resolve(r, Path::new(name), line, at(line, "reports")), Some(view.clone()), "{name}");
            assert_eq!(resolve(r, Path::new(name), render, at(render, "reports")), Some(twig.clone()), "{name}");
        }
        for name in ["console", "scripts/console", "ARTISAN", "notes.txt"] {
            assert_eq!(resolve(r, Path::new(name), line, at(line, "reports")), None, "{name}");
        }
    }

    #[test]
    fn template_source_case_keeps_specialized_routes_and_target_names() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let blade = touch(root, "resources/views/components/forms/input.blade.php");
        let twig = touch(root, "templates/Base.html.twig");
        for name in ["view.blade.php", "VIEW.BLADE.PHP", "view.Blade.Php"] {
            let line = "<x-forms.input type=\"text\" />";
            assert_eq!(resolve(root, Path::new(name), line, at(line, "forms")), Some(blade.clone()), "{name}");
        }
        for name in ["view.twig", "VIEW.TWIG", "view.Twig"] {
            let line = "{% extends 'Base.html.twig' %}";
            assert_eq!(resolve(root, Path::new(name), line, at(line, "Base")), Some(twig.clone()), "{name}");
            let wrong_case = "{% extends 'base.html.twig' %}";
            assert_eq!(resolve(root, Path::new(name), wrong_case, at(wrong_case, "base")), None);
        }
        let line = "<x-forms.input />";
        for name in ["view.php", "view.blade.php.bak", "view.twig.bak", "view.html"] {
            assert_eq!(resolve(root, Path::new(name), line, at(line, "forms")), None, "{name}");
        }
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
    fn quote_scanning_handles_escaped_backslashes_and_unfinished_strings() {
        let line = r"Route::view('/x\\', 'welcome');";
        assert_eq!(string_at(line, at(line, "welcome")).unwrap().0, "welcome");
        let line = r"view('unfinished\'";
        assert!(string_at(line, line.len()).is_none());
        let line = r"view('not\'a-template')";
        assert_eq!(string_at(line, at(line, "template")).unwrap().0, r"not\'a-template");
    }

    #[test]
    fn escaped_quotes_in_preceding_arguments_do_not_hide_template() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let target = touch(r, "resources/views/welcome.blade.php");
        let f = Path::new("routes/web.php");
        for line in [
            r"Route::view('/it\'s', 'welcome');",
            r#"Route::view("/say\"hello", "welcome");"#,
            r"@includeWhen($name === 'it\'s', 'welcome')",
        ] {
            assert_eq!(resolve(r, f, line, at(line, "welcome")), Some(target.clone()), "{line}");
        }
    }

    #[test]
    fn route_view_uses_second_argument_not_uri_or_data() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let target = touch(r, "resources/views/welcome.blade.php");
        touch(r, "resources/views/uri.blade.php");
        let f = Path::new("routes/web.php");
        for line in [
            "Route::view('uri', 'welcome');",
            "Route::view(route('home', ['x,y']), 'welcome', ['name' => 'welcome']);",
            "Route::view('/a,b', 'welcome');",
        ] {
            let col = at(line, "welcome");
            assert_eq!(resolve(r, f, line, col), Some(target.clone()), "{line}");
            assert!(hover(r, f, line, col).unwrap().contains("welcome.blade.php"));
        }
        let line = "Route::view('uri', 'welcome', ['name' => 'welcome']);";
        assert!(resolve(r, f, line, at(line, "uri")).is_none());
        assert!(resolve(r, f, line, line.rfind("welcome").unwrap()).is_none());
        let line = "FakeRoute::view('/x', 'welcome');";
        assert!(resolve(r, f, line, at(line, "welcome")).is_none());
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

    #[test]
    fn symfony_controller_render_opens_the_twig_template() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let t = touch(r, "templates/blog/index.html.twig");
        touch(r, "resources/views/blog/index.twig.blade.php");
        let f = r.join("src/Controller/BlogController.php");
        for l in [
            "return $this->render('blog/index.html.twig', ['a' => 1]);",
            "return $this->renderForm(\"blog/index.html.twig\");",
            "$html = $twig->render('blog/index.html.twig');",
        ] {
            assert_eq!(resolve(r, &f, l, at(l, "blog")), Some(t.clone()), "{l}");
        }
        let l = "return $this->render('blog/missing.html.twig');";
        assert_eq!(resolve(r, &f, l, at(l, "blog")), None);
        let l = "return $this->render('blog/index.'.$fmt.'.twig');";
        assert_eq!(resolve(r, &f, l, at(l, "blog")), None);
        let l = "return $this->fetch('blog/index.html.twig');";
        assert_eq!(resolve(r, &f, l, at(l, "blog")), None);
        let l = "return $this->render('../x.html.twig');";
        assert_eq!(resolve(r, &f, l, at(l, "x.html")), None);
    }
}
