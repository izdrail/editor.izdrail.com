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
    ("cspNonce", "<script @cspNonce>", "Outputs the `nonce=\"...\"` attribute for the request (spatie/laravel-csp)."),
    ("cspMetaTag", "@cspMetaTag", "Outputs the CSP policy as a `<meta>` tag (spatie/laravel-csp)."),
    ("stop", "@stop", "Ends a `@section` without yielding it (older equivalent of `@endsection`)."),
    ("append", "@append", "Ends a `@section` and appends it to the parent layout's section of the same name."),
    ("overwrite", "@overwrite", "Ends a `@section` and replaces the parent layout's section of the same name."),
    ("choice", "@choice('messages.apples', $count)", "Translates a key with pluralization."),
    ("elsecan", "@elsecan('ability', $model)", "Else-if branch of `@can`."),
    ("elsecannot", "@elsecannot('ability', $model)", "Else-if branch of `@cannot`."),
    ("elsecanany", "@elsecanany(['a', 'b'], $model)", "Else-if branch of `@canany`."),
    ("inertia", "@inertia", "Outputs the Inertia root `<div id=\"app\">` with the page data (Inertia.js)."),
    ("inertiaHead", "@inertiaHead", "Outputs the head tags set by Inertia's SSR (Inertia.js)."),
    ("routes", "@routes", "Outputs the Ziggy route table for JavaScript (tightenco/ziggy)."),
    ("teleport", "@teleport('#selector') ... @endteleport", "Renders the block elsewhere in the DOM, for example a modal at the end of `<body>` (Livewire)."),
    ("persist", "@persist('name') ... @endpersist", "Keeps the block alive across `wire:navigate` page changes (Livewire 3)."),
    ("script", "@script ... @endscript", "A script that runs with the Livewire component (Livewire 3)."),
    ("assets", "@assets ... @endassets", "Scripts or styles loaded once per page for a component (Livewire 3)."),
];

/// Closing directives and the opener each one ends.
const ENDS: &[(&str, &str)] = &[
    ("endforeach", "foreach"), ("endfor", "for"), ("endforelse", "forelse"), ("endwhile", "while"),
    ("endunless", "unless"), ("endisset", "isset"), ("endempty", "empty"), ("endauth", "auth"),
    ("endguest", "guest"), ("endcan", "can"), ("endcannot", "cannot"), ("endcanany", "canany"),
    ("endenv", "env"), ("endproduction", "production"), ("endswitch", "switch"), ("endpush", "push"),
    ("endprepend", "prepend"), ("endpushonce", "pushOnce"), ("endpushif", "pushIf"), ("endonce", "once"),
    ("endcomponent", "component"), ("endslot", "slot"), ("enderror", "error"), ("endphp", "php"),
    ("endverbatim", "verbatim"), ("endfragment", "fragment"), ("endsession", "session"),
    ("endteleport", "teleport"), ("endpersist", "persist"), ("endscript", "script"), ("endassets", "assets"),
];

/// Alpine and Livewire 3 attributes: (name, syntax, description).
pub(crate) const ATTRIBUTES: &[(&str, &str, &str)] = &[
    ("x-data", "x-data=\"{ open: false }\"", "Alpine: declares a component scope with reactive data."),
    ("x-init", "x-init=\"expression\"", "Alpine: runs when the element is initialised."),
    ("x-show", "x-show=\"expression\"", "Alpine: shows or hides the element (toggles `display: none`)."),
    ("x-bind", "x-bind:attr=\"expression\"  or  :attr=\"expression\"", "Alpine: binds an attribute to an expression."),
    ("x-on", "x-on:event.modifier=\"handler\"", "Alpine: listens for an event. The `@event` shorthand clashes with Blade directives in `.blade.php`, so prefer `x-on:`."),
    ("x-model", "x-model.modifier=\"property\"", "Alpine: two-way binds an input to data. Modifiers include `.lazy`, `.number`, `.debounce`."),
    ("x-text", "x-text=\"expression\"", "Alpine: sets the element's text content."),
    ("x-html", "x-html=\"expression\"", "Alpine: sets inner HTML. Only use it with trusted content."),
    ("x-ref", "x-ref=\"name\"", "Alpine: names the element so it is reachable as `$refs.name`."),
    ("x-cloak", "x-cloak", "Alpine: removed once Alpine has initialised; hide it with `[x-cloak] { display: none !important; }`."),
    ("x-transition", "x-transition  or  x-transition:enter=\"...\"", "Alpine: transitions an element shown or hidden with `x-show`."),
    ("x-for", "<template x-for=\"item in items\" :key=\"item.id\">", "Alpine: loops over a list. Must be on a `<template>` with one root element."),
    ("x-if", "<template x-if=\"condition\">", "Alpine: adds the content to the DOM only when true. Must be on a `<template>`."),
    ("x-effect", "x-effect=\"expression\"", "Alpine: re-runs the expression when the data it reads changes."),
    ("x-teleport", "<template x-teleport=\"selector\">", "Alpine: moves the template's content to another place in the DOM."),
    ("x-ignore", "x-ignore", "Alpine: does not initialise this element or its children."),
    ("x-id", "x-id=\"['name']\"", "Alpine: scopes generated ids for `$id('name')`."),
    ("x-modelable", "x-modelable=\"property\"", "Alpine: exposes a data property so a parent `x-model` can bind to it."),
    ("wire:click", "wire:click=\"method\"", "Livewire: calls a component method on click. Modifiers include `.prevent`, `.stop`."),
    ("wire:model", "wire:model=\"property\"", "Livewire 3: binds an input to a component property. Updates are sent with the next request; add `.live` to send on input, `.blur` on blur."),
    ("wire:submit", "wire:submit=\"method\"", "Livewire: calls a method when the form is submitted (the default submit is prevented)."),
    ("wire:loading", "wire:loading  /  wire:loading.class=\"...\"", "Livewire: shows or styles the element while a request is in flight. Scope it with `wire:target`."),
    ("wire:target", "wire:target=\"method\"", "Livewire: limits `wire:loading` or `wire:offline` to one action or property."),
    ("wire:poll", "wire:poll.5s=\"method\"", "Livewire: refreshes the component on an interval."),
    ("wire:navigate", "<a href=\"...\" wire:navigate>", "Livewire 3: SPA-style page navigation without a full reload."),
    ("wire:key", "wire:key=\"unique\"", "Livewire: unique key for items in a loop so the DOM can be tracked."),
    ("wire:init", "wire:init=\"method\"", "Livewire: calls a method when the element is initialised."),
    ("wire:ignore", "wire:ignore  /  wire:ignore.self", "Livewire: keeps the element out of DOM updates (for third-party widgets)."),
    ("wire:stream", "wire:stream=\"name\"", "Livewire: target for content sent with `$this->stream()`."),
    ("wire:transition", "wire:transition", "Livewire 3: transitions the element when it is added or removed."),
    ("wire:confirm", "wire:confirm=\"Are you sure?\"", "Livewire 3: asks for confirmation before a `wire:click` or `wire:submit` action runs."),
    ("wire:dirty", "wire:dirty  /  wire:dirty.class=\"...\"", "Livewire: shows or styles the element while the property has unsaved changes."),
    ("wire:offline", "wire:offline", "Livewire: shows the element while the browser is offline."),
    ("wire:cloak", "wire:cloak", "Livewire: hides the element until Livewire has initialised."),
];

/// The base attribute name (`x-on`, `wire:model`) when `col` is on an Alpine
/// or Livewire attribute in `line`: arguments (`:click`) and modifiers
/// (`.live`) are dropped.
fn attribute_at(line: &str, col: usize) -> Option<&str> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let is_attr = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | ':' | '.' | '_');
    let mut s = col;
    while let Some(c) = line[..s].chars().next_back() {
        if is_attr(c) { s -= c.len_utf8() } else { break }
    }
    let mut e = col;
    while let Some(c) = line[e..].chars().next() {
        if is_attr(c) { e += c.len_utf8() } else { break }
    }
    // The attribute must follow whitespace (or start the line): `foo-x-data` is not one.
    if s > 0 && !line[..s].ends_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let token = &line[s..e];
    let base = token.split('.').next()?;
    if let Some(rest) = base.strip_prefix("x-") {
        let name = rest.split(':').next()?;
        return Some(&base[..2 + name.len()]);
    }
    if base.starts_with("wire:") {
        return Some(base);
    }
    None
}

fn attribute_help(line: &str, col: usize) -> Option<String> {
    let name = attribute_at(line, col)?;
    let (n, syntax, desc) = ATTRIBUTES.iter().find(|(n, _, _)| *n == name)?;
    Some(format!("```html\n{syntax}\n```\n{desc}  \n`{n}`"))
}

fn directive(name: &str) -> Option<(String, String)> {
    if let Some((_, s, d)) = DIRECTIVES.iter().find(|(n, _, _)| *n == name) {
        return Some((s.to_string(), d.to_string()));
    }
    // Closing tags are matched without regard to case (`@endPushOnce`).
    let (end, open) = ENDS.iter().find(|(e, _)| e.eq_ignore_ascii_case(name))?;
    let (open_syntax, _) = DIRECTIVES.iter().find(|(n, _, _)| n == open).map(|(_, s, d)| (*s, *d))?;
    Some((format!("@{end}"), format!("Ends `@{open}`. Opened with: `{open_syntax}`")))
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

const VARIABLES: &[(&str, &str, &str)] = &[
    ("slot", "$slot", "The content between the component's tags (the default slot). Named slots are `$name` for `<x-slot:name>`. `$slot->isEmpty()` and `$slot->hasActualContent()` check it."),
    ("attributes", "$attributes", "Attributes passed to the component that are not declared in `@props`. Use `{{ $attributes }}` on the root element, `$attributes->merge(['class' => 'p-4'])`, `->class([...])`, `->only(...)`, `->except(...)`, `->whereStartsWith('wire:model')`, `->has('x')`, `->get('x')`."),
    ("component", "$component", "The component instance (class components only)."),
    ("loop", "$loop", "Inside `@foreach` / `@forelse`: `->index` (0-based), `->iteration` (1-based), `->remaining`, `->count`, `->first`, `->last`, `->even`, `->odd`, `->depth`, `->parent`."),
    ("errors", "$errors", "The validation error bag. `$errors->any()`, `$errors->has('f')`, `$errors->first('f')`, `$errors->get('f')`."),
    ("message", "$message", "The error text inside `@error('field') ... @enderror`."),
];

/// Alpine and Livewire magic properties usable inside attribute
/// expressions (`x-on:click="$dispatch('saved')"`, `wire:click="$wire.save()"`).
const MAGICS: &[(&str, &str, &str)] = &[
    ("el", "$el", "Alpine: the element the current expression is on."),
    ("refs", "$refs", "Alpine: elements in this component tagged with `x-ref`, read as `$refs.name`."),
    ("store", "$store", "Alpine: a global store registered with `Alpine.store('name', ...)`; read and write as `$store.name.prop` from any component."),
    ("watch", "$watch", "Alpine: `$watch('prop', (value, old) => ...)` runs the callback when the component property changes."),
    ("dispatch", "$dispatch", "Alpine: `$dispatch('saved', { id: 1 })` dispatches a bubbling CustomEvent; listen with `x-on:saved`, `@saved.window` outside the component, or `#[On('saved')]` in Livewire."),
    ("nextTick", "$nextTick", "Alpine: `$nextTick(() => ...)` runs the callback after Alpine has applied the pending DOM updates."),
    ("root", "$root", "Alpine: the root element of the current component (the one carrying `x-data`)."),
    ("id", "$id", "Alpine: `$id('input')` returns a page-unique id inside an `x-id` scope, for label/input pairs."),
    ("data", "$data", "Alpine: the merged `x-data` object of the current component."),
    ("wire", "$wire", "Livewire (inside Alpine expressions): the component. `$wire.count` reads a property, `$wire.save()` calls a method, `$wire.set('count', 1)` and `$wire.toggle('open')` write, `$wire.$refresh()` re-renders, `$wire.$parent` reaches the parent component."),
    ("js", "$js", "Livewire 3: a function registered with `$js('name', fn)` in the component's `@script` block, called from Alpine as `$wire.$js.name()`."),
    ("entangle", "$entangle", "Livewire: `$entangle('prop')` two-way binds an Alpine property to a Livewire property. Livewire 3 usually wants `wire:model` or `@entangle` in Blade instead."),
];

/// True when byte `col` sits inside a quoted string on this line, but not
/// inside a Blade echo `{{ ... }}` / `{!! ... !!}` within it: magic
/// properties live in Alpine/Livewire attribute expressions, so a plain
/// Blade `{{ $data }}` must not be explained as Alpine's `$data`.
fn in_expression_string(line: &str, col: usize) -> bool {
    let bytes = line.as_bytes();
    let mut open: Option<(usize, u8)> = None;
    let mut i = 0;
    while i < col.min(bytes.len()) {
        let b = bytes[i];
        if b == b'\\' {
            i += 2;
            continue;
        }
        match open {
            Some((_, q)) if b == q => open = None,
            None if b == b'\'' || b == b'"' => open = Some((i, b)),
            _ => {}
        }
        i += 1;
    }
    let Some((start, _)) = open else { return false };
    let within = &line[start..col];
    !within.contains("{{") && !within.contains("{!!")
}

/// Help for an Alpine/Livewire magic property (`$dispatch`, `$wire`, ...)
/// in an attribute expression.
fn magic_help(line: &str, col: usize, name: &str) -> Option<String> {
    let (_, syntax, desc) = MAGICS.iter().find(|(n, _, _)| *n == name)?;
    in_expression_string(line, col)
        .then(|| format!("```js\n{syntax}\n```\n{desc}"))
}

fn variable_at(line: &str, col: usize) -> Option<&str> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut s = col;
    while let Some(c) = line[..s].chars().next_back() {
        if is_name(c) { s -= c.len_utf8() } else { break }
    }
    let mut e = col;
    while let Some(c) = line[e..].chars().next() {
        if is_name(c) { e += c.len_utf8() } else { break }
    }
    (s < e && line[..s].ends_with('$')).then(|| &line[s..e])
}

/// Help for `$slot`, `$attributes`, `$loop`... and, inside a component view,
/// for a variable declared in the file's own `@props`.
fn variable_help(root: Option<&Path>, file: &Path, line: &str, col: usize) -> Option<String> {
    let name = variable_at(line, col)?;
    let in_component = root.is_some_and(|r| file.starts_with(r.join("resources/views/components")));
    if let Some((_, syntax, desc)) = VARIABLES.iter().find(|(n, _, _)| *n == name) {
        // `$slot`, `$attributes`, `$component` only mean something in components.
        let component_only = matches!(name, "slot" | "attributes" | "component");
        if !component_only || in_component || root.is_none() {
            return Some(format!("```blade\n{syntax}\n```\n{desc}"));
        }
    }
    if in_component {
        let prop = || -> Option<String> {
            let meta = std::fs::symlink_metadata(file).ok()?;
            (meta.is_file() && meta.len() <= 1 << 20).then_some(())?;
            let text = std::fs::read_to_string(file).ok()?;
            let n = crate::blade_props::prop_line(&text, name)?;
            let decl = text.lines().nth(n)?.trim();
            Some(format!("**Prop** `${name}`\n```php\n{decl}\n```\nline {} of this component (from the saved file)", n + 1))
        };
        if let Some(md) = prop() {
            return Some(md);
        }
    }
    magic_help(line, col, name)
}

/// `(marker, name)` when the caret is on a component tag name on this
/// line: `<x-alert`, `</x-forms.input`, `<livewire:admin.user-profile` (or
/// their closing tags). The caret must be inside the name itself.
fn component_tag_at(line: &str, col: usize) -> Option<(&'static str, &str)> {
    let col = col.min(line.len());
    let mut from = 0;
    while let Some(rel) = line[from..].find('<') {
        let lt = from + rel;
        let mut p = lt + 1;
        if line[p..].starts_with('/') {
            p += 1;
        }
        for marker in ["x-", "livewire:"] {
            if let Some(rest) = line[p..].strip_prefix(marker) {
                let name_len = rest
                    .bytes()
                    .take_while(|b| {
                        b.is_ascii_alphanumeric()
                            || matches!(b, b'.' | b'-' | b'_')
                    })
                    .count();
                let start = p + marker.len();
                let end = start + name_len;
                if name_len > 0 && (start..end).contains(&col) {
                    return Some((marker, &line[start..end]));
                }
            }
        }
        from = lt + 1;
    }
    None
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
    if let Some(md) = variable_help(root, file, line, at - line_start) {
        return Some(md);
    }
    if let Some(md) = attribute_help(line, at - line_start) {
        return Some(md);
    }
    if let Some((marker, tag)) = component_tag_at(line, at - line_start) {
        let root = root?;
        let target = if marker == "x-" {
            crate::template_refs::blade_component(root, tag)
        } else {
            crate::template_refs::livewire_component(root, tag)
        }?;
        let shown = target.strip_prefix(root).unwrap_or(&target).display();
        let mut md = format!("**`<{marker}{tag}>`**\n{shown}");
        if let Ok(text) = std::fs::read_to_string(&target) {
            if let Some(props) =
                text.lines().find(|l| l.trim_start().starts_with("@props"))
            {
                md.push_str(&format!("\n```blade\n{}\n```", props.trim()));
            }
        }
        return Some(md);
    }
    let root = root?;
    let Some((tag, attr)) = crate::blade_props::tag_and_attr(window, at) else {
        return crate::livewire_props::help(root, file, line, at - line_start);
    };
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
    fn alpine_and_livewire_attribute_hover() {
        let l = r#"<button x-on:click.prevent="open = true" wire:model.live="q" :class="a">"#;
        let at = |needle: &str, off: usize| l.find(needle).unwrap() + off;
        assert_eq!(attribute_at(l, at("x-on", 1)), Some("x-on"));
        assert_eq!(attribute_at(l, at("click", 2)), Some("x-on"));
        assert_eq!(attribute_at(l, at("prevent", 2)), Some("x-on"));
        assert_eq!(attribute_at(l, at("wire:model", 7)), Some("wire:model"));
        assert_eq!(attribute_at(l, at("live", 1)), Some("wire:model"));
        assert_eq!(attribute_at(l, at(":class", 2)), None);
        assert_eq!(attribute_at("<div x-data>", 7), Some("x-data"));
        assert_eq!(attribute_at("foo-x-data", 6), None);
        assert_eq!(attribute_at("<div>", 3), None);
        assert_eq!(attribute_at("é x-ref", 5), Some("x-ref"));
        let h = attribute_help(l, at("wire:model", 2)).unwrap();
        assert!(h.contains("Livewire 3") && h.contains("wire:model"), "{h}");
        assert!(attribute_help("<a x-unknown>", 6).is_none());
        let f = Path::new("a.blade.php");
        assert!(help(None, f, "<div x-show=\"open\">", 8).unwrap().contains("display: none"));
    }

    #[test]
    fn attribute_table_is_unique_and_well_formed() {
        for (i, (n, s, _)) in ATTRIBUTES.iter().enumerate() {
            assert!(s.contains(n) || n.starts_with("x-on") || n.starts_with("x-bind"), "{n}");
            assert!(ATTRIBUTES[i + 1..].iter().all(|(m, _, _)| m != n), "dup {n}");
        }
    }

    #[test]
    fn closing_tags_and_new_directives() {
        let (s, d) = directive("endforeach").unwrap();
        assert_eq!(s, "@endforeach");
        assert!(d.contains("Ends `@foreach`") && d.contains("@foreach ($items as $item)"), "{d}");
        assert!(directive("endPushOnce").is_some());
        assert!(directive("ENDPHP").is_some());
        assert!(directive("endnope").is_none());
        for name in ["stop", "append", "choice", "inertia", "persist", "cspNonce"] {
            assert!(directive(name).is_some(), "{name}");
        }
        // Every closing tag points at a real opener, and the closers start with "end".
        for (e, o) in ENDS {
            assert!(e.starts_with("end"), "{e}");
            assert!(DIRECTIVES.iter().any(|(n, _, _)| n == o), "{e} -> {o}");
        }
        let f = Path::new("a.blade.php");
        let h = help(None, f, "@endauth", 3).unwrap();
        assert!(h.contains("```blade\n@endauth\n```") && h.contains("Ends `@auth`"), "{h}");
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
    fn variable_hover() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let comp = r.join("resources/views/components/btn.blade.php");
        std::fs::create_dir_all(comp.parent().unwrap()).unwrap();
        std::fs::write(&comp, "@props([\n 'type' => 'button',\n])\n<button {{ $attributes }}>{{ $slot }}</button>\n").unwrap();
        let page = r.join("resources/views/page.blade.php");
        let l = "<button {{ $attributes }}>{{ $slot }} {{ $type }} @foreach($a as $b) {{ $loop->index }}";
        let at = |m: &str| l.find(m).unwrap() + 2;
        assert!(help(Some(r), &comp, l, at("attributes")).unwrap().contains("merge"));
        assert!(help(Some(r), &comp, l, at("slot")).unwrap().contains("default slot"));
        let h = help(Some(r), &comp, l, at("type")).unwrap();
        assert!(h.contains("'type' => 'button'") && h.contains("line 2"), "{h}");
        // $loop works anywhere, $slot only in components.
        assert!(help(Some(r), &page, l, at("loop")).unwrap().contains("iteration"));
        assert!(help(Some(r), &page, l, at("slot")).is_none());
        assert!(help(Some(r), &comp, l, at("a as")).is_none());
        assert_eq!(variable_at("$", 1), None);
        assert_eq!(variable_at("é$ab x", 4), Some("ab"));
        assert_eq!(variable_at("é$日 x", 5), None);
    }

    #[test]
    fn component_tag_hover() {
        let l = "  <x-forms.input type=\"text\" />";
        assert_eq!(
            component_tag_at(l, l.find("forms").unwrap()),
            Some(("x-", "forms.input"))
        );
        assert_eq!(component_tag_at(l, l.find("type").unwrap()), None);
        assert_eq!(component_tag_at("</x-alert", 4), Some(("x-", "alert")));
        let l = "<livewire:admin.user-profile />";
        assert_eq!(
            component_tag_at(l, 15),
            Some(("livewire:", "admin.user-profile"))
        );
        assert_eq!(component_tag_at("<div class=", 5), None);
        // help() resolves the component file and shows its @props.
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let dir = r.join("resources/views/components");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("alert.blade.php"),
            "@props(['type' => 'info'])\n<div>{{ $slot }}</div>\n",
        )
        .unwrap();
        let view = r.join("resources/views/show.blade.php");
        let w = "<x-alert type=\"info\" />";
        let md = help(Some(r), &view, w, 3).unwrap();
        assert!(md.contains("<x-alert>"), "{md}");
        assert!(md.contains("resources/views/components/alert.blade.php"), "{md}");
        assert!(md.contains("@props(['type' => 'info'])"), "{md}");
        // Unknown component: no hover, no panic.
        assert!(help(Some(r), &view, "<x-missing />", 3).is_none());
    }

    #[test]
    fn magic_property_hover() {
        let f = Path::new("resources/views/a.blade.php");
        let l = r#"<button x-on:click="$dispatch('saved')" wire:click="$wire.save()">x</button>"#;
        let h = help(None, f, l, l.find("$dispatch").unwrap() + 3).unwrap();
        assert!(h.contains("$dispatch") && h.contains("CustomEvent"), "{h}");
        let h = help(None, f, l, l.find("$wire").unwrap() + 3).unwrap();
        assert!(h.contains("Livewire") && h.contains("$wire.save()"), "{h}");
        let l = r#"<div x-data="{ open: $store.nav }">x</div>"#;
        assert!(help(None, f, l, l.find("$store").unwrap() + 2)
            .unwrap()
            .contains("Alpine.store"));
        // A Blade echo inside an attribute is not an Alpine expression.
        let l = r#"<p class="{{ $data }}">x</p>"#;
        assert!(help(None, f, l, l.find("$data").unwrap() + 2).is_none());
        // Outside any quoted string: nothing.
        assert!(help(None, f, "{{ $watch }}", 4).is_none());
        // A component prop beats a magic name.
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let comp = r.join("resources/views/components/btn.blade.php");
        std::fs::create_dir_all(comp.parent().unwrap()).unwrap();
        std::fs::write(&comp, "@props(['data' => null])\n").unwrap();
        let l = r#"<p class="{{ $data }}">x</p>"#;
        let h = help(Some(r), &comp, l, l.find("$data").unwrap() + 2).unwrap();
        assert!(h.contains("Prop") && h.contains("'data' => null"), "{h}");
        assert!(in_expression_string(r#"x-on:click="$el.focus()"#, 16));
        assert!(!in_expression_string("plain $el text", 8));
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
