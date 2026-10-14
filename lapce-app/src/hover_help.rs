//! Local hover help for Blade files, shown before any language server is
//! asked:
//! - a Blade directive (`@foreach`, `@can`, `@vite`, ...): syntax and a short
//!   description from a built-in table of Laravel directives;
//! - an attribute of an `<x-...>` tag: the component's declaration of that
//!   prop (the `@props` or constructor line) and where it lives.
//!
//! Static text only. The table follows Laravel 10/11 Blade; directives it does
//! not know (custom ones from `Blade::directive`) get no hover.
use std::path::Path;

const DIRECTIVES: &[(&str, &str, &str)] = &[
    ("if", "@if (cond) ... @elseif (cond) ... @else ... @endif", "Conditional block."),
    ("elseif", "@elseif (cond)", "Else-if branch of an `@if`."),
    ("else", "@else", "Else branch of `@if`, `@unless`, `@isset`, `@can` and similar."),
    ("endif", "@endif", "Ends an `@if`."),
    ("unless", "@unless (cond) ... @endunless", "Runs when the condition is false."),
    ("isset", "@isset($var) ... @endisset", "Runs when the variable is set and not null."),
    ("empty", "@empty($var) ... @endempty", "Runs when `empty($var)` is true. Inside `@forelse`, the no-items branch."),
    ("auth", "@auth('guard') ... @endauth", "Runs for authenticated users."),
    ("guest", "@guest('guard') ... @endguest", "Runs for unauthenticated users."),
    ("production", "@production ... @endproduction", "Runs only when the app environment is `production`."),
    ("env", "@env('local') ... @endenv", "Runs when the app environment matches."),
    ("hasSection", "@hasSection('name')", "True when the section has content."),
    ("sectionMissing", "@sectionMissing('name')", "True when the section has no content."),
    ("switch", "@switch($v) @case(1) ... @break @default ... @endswitch", "Switch statement."),
    ("case", "@case(value)", "Case of an `@switch`."),
    ("default", "@default", "Default case of an `@switch`."),
    ("for", "@for ($i = 0; $i < n; $i++) ... @endfor", "Plain `for` loop."),
    ("foreach", "@foreach ($items as $item) ... @endforeach", "Loop over a collection. `$loop` is available inside."),
    ("forelse", "@forelse ($items as $item) ... @empty ... @endforelse", "`@foreach` with an `@empty` branch for no items."),
    ("while", "@while (cond) ... @endwhile", "While loop."),
    ("continue", "@continue / @continue(cond)", "Skips to the next loop iteration."),
    ("break", "@break / @break(cond)", "Leaves the loop (or `@case`)."),
    ("include", "@include('view', ['k' => $v])", "Includes another view; the parent's variables are shared."),
    ("includeIf", "@includeIf('view', [...])", "Includes the view only if it exists."),
    ("includeWhen", "@includeWhen(cond, 'view', [...])", "Includes the view when the condition is true."),
    ("includeUnless", "@includeUnless(cond, 'view', [...])", "Includes the view when the condition is false."),
    ("includeFirst", "@includeFirst(['a', 'b'], [...])", "Includes the first view that exists."),
    ("each", "@each('view', $items, 'item', 'empty-view')", "Renders a view for each item."),
    ("extends", "@extends('layouts.app')", "Makes this view a child of a layout."),
    ("section", "@section('name') ... @endsection  or  @section('name', 'text')", "Defines content for a layout's `@yield`."),
    ("endsection", "@endsection", "Ends a `@section`."),
    ("show", "@show", "Ends a `@section` and yields it immediately."),
    ("yield", "@yield('name', 'default')", "Outputs a section in a layout."),
    ("parent", "@parent", "Inside a `@section`: the parent layout's content for that section."),
    ("stack", "@stack('name')", "Outputs everything pushed onto the stack."),
    ("push", "@push('name') ... @endpush", "Adds content to a stack (appended)."),
    ("prepend", "@prepend('name') ... @endprepend", "Adds content to the start of a stack."),
    ("pushOnce", "@pushOnce('name') ... @endPushOnce", "Pushes only the first time it is reached."),
    ("pushIf", "@pushIf(cond, 'name') ... @endPushIf", "Pushes when the condition is true."),
    ("once", "@once ... @endonce", "Renders the block only once per render."),
    ("component", "@component('view', [...]) ... @endcomponent", "Class-less component call, older syntax. Prefer `<x-...>`."),
    ("slot", "@slot('name') ... @endslot", "Named slot for `@component`. With `<x-...>` use `<x-slot:name>`."),
    ("props", "@props(['type' => 'text', 'disabled' => false])", "Declares the attributes of an anonymous component. Other attributes go to `$attributes`."),
    ("aware", "@aware(['color' => 'gray'])", "Reads props from a parent component."),
    ("can", "@can('ability', $model) ... @endcan", "Runs when the user passes the gate/policy check."),
    ("cannot", "@cannot('ability', $model)", "Runs when the user fails the check."),
    ("canany", "@canany(['a', 'b'], $model)", "Runs when any ability passes."),
    ("csrf", "@csrf", "Outputs the hidden CSRF token input for forms."),
    ("method", "@method('PUT')", "Outputs the hidden `_method` input for form method spoofing."),
    ("error", "@error('field') ... $message ... @enderror", "Runs when the field has a validation error; `$message` holds it."),
    ("class", "@class(['p-4', 'font-bold' => $bold])", "Builds a class attribute from an array."),
    ("style", "@style(['color: red' => $red])", "Builds a style attribute from an array."),
    ("checked", "@checked(cond)", "Outputs `checked` when the condition is true."),
    ("selected", "@selected(cond)", "Outputs `selected` when the condition is true."),
    ("disabled", "@disabled(cond)", "Outputs `disabled` when the condition is true."),
    ("readonly", "@readonly(cond)", "Outputs `readonly` when the condition is true."),
    ("required", "@required(cond)", "Outputs `required` when the condition is true."),
    ("json", "@json($data)", "Outputs the value as JSON, for use in JavaScript."),
    ("js", "@js($data)", "Outputs a JavaScript expression for the value (escaped for HTML)."),
    ("vite", "@vite(['resources/css/app.css', 'resources/js/app.js'])", "Outputs the Vite tags for the entry points."),
    ("viteReactRefresh", "@viteReactRefresh", "Outputs the React refresh preamble for Vite."),
    ("php", "@php ... @endphp", "Raw PHP block."),
    ("verbatim", "@verbatim ... @endverbatim", "Leaves `{{ }}` untouched, for JavaScript templates."),
    ("lang", "@lang('messages.welcome')", "Translates a key."),
    ("dd", "@dd($var)", "Dumps the variable and stops."),
    ("dump", "@dump($var)", "Dumps the variable."),
    ("inject", "@inject('metrics', 'App\\Services\\MetricsService')", "Resolves a service from the container into a variable."),
    ("use", "@use('App\\Models\\Flight')", "Imports a class into the compiled view (Laravel 11)."),
    ("livewire", "@livewire('component', [...])", "Renders a Livewire component."),
    ("livewireStyles", "@livewireStyles", "Outputs Livewire's CSS (Livewire 2)."),
    ("livewireScripts", "@livewireScripts", "Outputs Livewire's JavaScript (Livewire 2)."),
    ("auth", "@auth('guard') ... @endauth", "Runs for authenticated users."),
    ("session", "@session('key') ... $value ... @endsession", "Runs when the session key exists; `$value` holds it (Laravel 11)."),
    ("fragment", "@fragment('name') ... @endfragment", "Marks a part of the view that can be returned alone (htmx/Turbo)."),
    ("csp", "@cspNonce", "Outputs the CSP nonce attribute value."),
];

fn directive(name: &str) -> Option<(&'static str, &'static str)> {
    DIRECTIVES.iter().find(|(n, _, _)| *n == name).map(|(_, s, d)| (*s, *d))
}

/// The directive name when `col` is on `@name` in `line`.
fn directive_at(line: &str, col: usize) -> Option<&str> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut s = col;
    while let Some(c) = line[..s].chars().next_back() {
        if is_name(c) { s -= c.len_utf8() } else { break }
    }
    if s == 0 || !line[..s].ends_with('@') {
        return None;
    }
    let at = s - 1;
    // `@@name` is an escaped literal, and `foo@bar` is an email address.
    if line[..at].ends_with('@') || line[..at].chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let mut e = col;
    while let Some(c) = line[e..].chars().next() {
        if is_name(c) { e += c.len_utf8() } else { break }
    }
    (e > s).then(|| &line[s..e])
}

/// Markdown for the hover at byte `col` of the line the window ends on.
/// `window` is the text of up to 30 lines above plus the hovered line (for
/// multi-line component tags); `at` is the hover byte offset inside it.
pub fn help(root: Option<&Path>, file: &Path, window: &str, at: usize) -> Option<String> {
    if !file.file_name()?.to_str()?.ends_with(".blade.php") {
        return None;
    }
    let mut at = at.min(window.len());
    while !window.is_char_boundary(at) {
        at -= 1;
    }
    let line_start = window[..at].rfind('\n').map_or(0, |n| n + 1);
    let line_end = window[at..].find('\n').map_or(window.len(), |n| at + n);
    let line = &window[line_start..line_end];
    if let Some(name) = directive_at(line, at - line_start) {
        let (syntax, desc) = directive(name)?;
        return Some(format!("```blade\n{syntax}\n```\n{desc}"));
    }
    let root = root?;
    let (tag, attr) = crate::blade_props::tag_and_attr(window, at)?;
    let (target, n) = crate::blade_props::resolve(root, file, window, at)?;
    let text = std::fs::read_to_string(&target).ok()?;
    let decl = text.lines().nth(n)?.trim();
    let shown = target.strip_prefix(root).unwrap_or(&target).display();
    Some(format!(
        "**`<x-{tag}>`** prop `{attr}`\n```php\n{decl}\n```\n{shown}:{}",
        n + 1
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directive_lookup() {
        let l = "  @foreach ($users as $u)";
        assert_eq!(directive_at(l, 4), Some("foreach"));
        assert_eq!(directive_at(l, 2), None);
        assert_eq!(directive_at(l, 3), Some("foreach"));
        assert_eq!(directive_at(l, 12), None);
        assert_eq!(directive_at("a@b.com", 3), None);
        assert_eq!(directive_at("@@if", 3), None);
        assert_eq!(directive_at("é @if", 5), Some("if"));
        assert_eq!(directive_at("@", 1), None);
        assert!(directive("vite").is_some());
        assert!(directive("nope").is_none());
    }

    #[test]
    fn table_has_no_duplicates_with_different_text() {
        for (i, (n, s, _)) in DIRECTIVES.iter().enumerate() {
            assert!(s.contains(&format!("@{n}")), "{n}");
            for (m, s2, _) in &DIRECTIVES[i + 1..] {
                if m == n {
                    assert_eq!(s, s2);
                }
            }
        }
    }

    #[test]
    fn hover_for_directive_and_prop() {
        let d = tempfile::tempdir().unwrap();
        let v = d.path().join("resources/views/components");
        std::fs::create_dir_all(&v).unwrap();
        std::fs::write(v.join("btn.blade.php"), "@props([\n 'type' => 'button',\n])\n").unwrap();
        let f = d.path().join("resources/views/p.blade.php");
        let t = "@can('x')\n<x-btn\n  type=\"submit\" />";
        let h = help(Some(d.path()), &f, t, 2).unwrap();
        assert!(h.contains("@can('ability'"), "{h}");
        let at = t.find("type").unwrap() + 1;
        let h = help(Some(d.path()), &f, t, at).unwrap();
        assert!(h.contains("'type' => 'button'") && h.contains("btn.blade.php:2"), "{h}");
        assert!(help(Some(d.path()), Path::new("a.php"), t, 2).is_none());
        assert!(help(None, &f, t, at).is_none());
        let unknown = help(Some(d.path()), &f, "@mycustom", 2);
        assert!(unknown.is_none());
    }

    #[test]
    fn never_panics() {
        let f = Path::new("a.blade.php");
        let t = "é@é <x-é é=\"é\" @";
        for i in 0..t.len() + 3 {
            let _ = help(None, f, t, i);
            let _ = help(Some(Path::new("/nonexistent")), f, t, i);
        }
    }
}
