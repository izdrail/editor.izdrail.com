//! Project-aware completion items: inside `route('...')` offer the app's
//! named routes, inside `config('...')` its config keys, inside `env('...')`
//! the env keys, inside `view('...')` its Blade view names, inside
//! `__('...')` its translation keys, and inside `wire:model="..."` /
//! `wire:click="..."` in a Livewire view the paired component's public
//! properties / methods. In a `.blade.php` view, typing `$` offers the
//! variables the view defines above the cursor, typing `<x-`
//! offers the app's Blade component tags, and `asset('...')` /
//! `secure_asset('...')` / `mix('...')` complete files under
//! `public/`, and `@can` / `->can()` / `Gate::allows('...')`
//! complete Gate abilities from policies and `Gate::define`. Same
//! static
//! scans as the palettes (nothing is executed), cached 30 s per workspace
//! (per component class for Livewire) so completion stays cheap on every
//! keystroke.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use lsp_types::{CompletionItem, CompletionItemKind};

const TTL: Duration = Duration::from_secs(30);
const MAX_ROUTE_FILES: usize = 60;
const MAX_FILE: u64 = 1 << 20;
const MAX_DEPTH: usize = 6;
const MAX_VIEW_FILES: usize = 1000;
const MAX_NAMES: usize = 2000;
const MAX_ITEMS: usize = 500;

/// Which project string is being typed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Route,
    Config,
    Env,
    View,
    /// Blade `@section('...')` / `@yield('...')`.
    Section,
    /// Blade `@push('...')` / `@stack('...')`.
    Stack,
    /// Translation key: `__`, `trans`, `trans_choice`, Blade `@lang` /
    /// `@choice`, `Lang::get` / `Lang::has` / `Lang::choice`.
    Lang,
    /// Livewire `wire:model="..."` (any modifier): a property of the
    /// component class paired with the view.
    LivewireModel,
    /// Livewire `wire:click="..."` and the other event attributes: a
    /// method of the component class paired with the view.
    LivewireAction,
    /// Blade component tag `<x-<partial>` (dots for nested components).
    ComponentTag,
    /// Livewire component tag `<livewire:name` (dots for nesting).
    LivewireTag,
    /// Livewire directive `@livewire('name')`: a component name.
    LivewireName,
    /// `asset('...')` / `secure_asset('...')` / `mix('...')`: a path
    /// under `public/`.
    Asset,
    /// Gate ability: `@can('...')`, `$user->can('...')`,
    /// `Gate::allows('...')` and friends.
    Gate,
}

/// The context at the end of `line` (the text before the cursor): the last
/// `word(` whose argument string is still open. Quote escapes are not read;
/// `route('it\'s')` is rare and simply falls out of context.
pub fn context(line: &str) -> Option<Kind> {
    // Livewire `wire:<event>(.modifier)*="..."`: an open attribute value.
    // No parenthesis involved, so this runs before the call-argument scan.
    let mut hay = line;
    while let Some(at) = hay.rfind("wire:") {
        // A name-ish char before `wire:` means it is part of a longer
        // attribute (`data-wire:...` or Alpine `x-on:...wire:...`).
        if hay[..at]
            .chars()
            .next_back()
            .is_some_and(|c| {
                c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '-' | '.')
            })
        {
            hay = &hay[..at];
            continue;
        }
        let after = &hay[at + 5..];
        let attr_len = after
            .find(|c: char| {
                !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            })
            .unwrap_or(after.len());
        let attr = &after[..attr_len];
        let event = attr.split('.').next().unwrap_or("");
        let rest = &after[attr_len..];
        let quoted = rest
            .strip_prefix('=')
            .and_then(|r| r.chars().next().map(|q| (q, &r[q.len_utf8()..])));
        if let Some((q, value)) = quoted {
            if (q == '"' || q == '\'') && !value.contains(q) {
                if event == "model" {
                    return Some(Kind::LivewireModel);
                }
                if crate::livewire_handlers::EVENTS.contains(&event) {
                    return Some(Kind::LivewireAction);
                }
            }
        }
        break;
    }
    // Livewire component tag `<livewire:admin.user-profile`: the tail after
    // the marker is name characters only.
    if let Some(at) = line.rfind("<livewire:") {
        let tail = &line[at + "<livewire:".len()..];
        if tail
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        {
            return Some(Kind::LivewireTag);
        }
    }
    // Blade component tag `<x-<partial>`: the tail of the line is tag
    // characters (letters, digits, `.`, `-`, `_`) right after `<x-`.
    // Runs before the call-argument scan so a `(...)` earlier on the
    // line cannot misroute it.
    {
        let b = line.as_bytes();
        let mut i = b.len();
        while i > 0
            && matches!(b[i - 1], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'-' | b'_')
        {
            i -= 1;
        }
        // The marker's `x-` is itself in the tail class, so the tail
        // starts right after `<` (i is a char boundary: the scan only
        // consumed ASCII bytes, and a non-class byte is never a UTF-8
        // continuation byte that ends mid-tail).
        if i >= 1
            && line.get(..i).is_some_and(|h| h.ends_with('<'))
            && line[i..].starts_with("x-")
            && !line[i..].starts_with("x-slot")
        {
            let ident_end = line[..i - 1]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
            if !ident_end {
                return Some(Kind::ComponentTag);
            }
        }
    }
    let paren = line.rfind('(')?;
    let before = &line[..paren];
    // Still inside the argument string: quote state at end of line is open.
    let mut in_str: Option<char> = None;
    for c in line[paren + 1..].chars() {
        match in_str {
            None => {
                if c == '\'' || c == '"' {
                    in_str = Some(c);
                } else if c == ')' {
                    return None;
                }
            }
            Some(q) => {
                if c == q {
                    in_str = None;
                }
            }
        }
    }
    in_str?;
    let head = before.trim_end();
    let word_start = head
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map(|x| x + 1)
        .unwrap_or(0);
    let word = &head[word_start..];
    let blade = head[..word_start].ends_with('@');
    Some(match word {
        "route" | "to_route" | "routeIs" | "path" | "generateUrl"
        | "redirectToRoute" | "generate" => Kind::Route,
        // Route::has('...') - but not $collection->has('...').
        "has" if head[..word_start].ends_with("Route::") => Kind::Route,
        "config" => Kind::Config,
        "env" | "getenv" => Kind::Env,
        "view" => Kind::View,
        "asset" | "secure_asset" | "mix" => Kind::Asset,
        "can" | "cannot" | "canany" if blade => Kind::Gate,
        "can" | "cannot" | "authorize" if head[..word_start].ends_with("->") => {
            Kind::Gate
        }
        "allows" | "denies" | "any" | "authorize" | "check" | "none"
            if head[..word_start].ends_with("Gate::") =>
        {
            Kind::Gate
        }
        // Blade directives take view names too.
        "extends" | "include" | "includeIf" | "includeWhen" | "includeFirst"
        | "each" if blade => Kind::View,
        "section" | "yield" if blade => Kind::Section,
        "push" | "pushOnce" | "stack" if blade => Kind::Stack,
        "__" | "trans" | "trans_choice" => Kind::Lang,
        "lang" | "choice" if blade => Kind::Lang,
        "livewire" if blade => Kind::LivewireName,
        "get" | "has" | "choice" if head[..word_start].ends_with("Lang::") => {
            Kind::Lang
        }
        _ => return None,
    })
}

#[derive(Clone)]
struct Lists {
    root: PathBuf,
    at: Instant,
    routes: Vec<String>,
    configs: Vec<String>,
    envs: Vec<String>,
    views: Vec<String>,
    sections: Vec<String>,
    stacks: Vec<String>,
    langs: Vec<String>,
    components: Vec<String>,
    assets: Vec<String>,
    gates: Vec<String>,
}

static CACHE: OnceLock<Mutex<Option<Lists>>> = OnceLock::new();

/// `->name('...')` values in routes/*.php (Laravel).
fn route_names(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(root.join("routes")) else { return out };
    let mut files: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "php"))
        .collect();
    files.sort();
    files.truncate(MAX_ROUTE_FILES);
    for f in files {
        let Ok(meta) = std::fs::metadata(&f) else { continue };
        if !meta.is_file() || meta.len() > MAX_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for (i, _) in text.match_indices("->name(") {
            let rest = text[i + 7..].trim_start();
            let Some(q) = rest.chars().next().filter(|c| *c == '\'' || *c == '"')
            else {
                continue;
            };
            let Some(end) = rest[1..].find(q) else { continue };
            let name = &rest[1..1 + end];
            if !name.is_empty()
                && name.len() <= 100
                && !out.iter().any(|n: &String| n == name)
                && out.len() < MAX_NAMES
            {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Blade view names (`pages/home.blade.php` -> `pages.home`).
fn view_names(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, depth: usize, base: &Path, out: &mut Vec<String>) {
        if depth > MAX_DEPTH || out.len() >= MAX_VIEW_FILES {
            return;
        }
        if depth == 0
            && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink())
        {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.filter_map(Result::ok) {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let p = e.path();
            if ft.is_dir() {
                walk(&p, depth + 1, base, out);
            } else if ft.is_file() {
                let Some(name) = p.file_name().and_then(|n| n.to_str()) else { continue };
                let stem = name
                    .strip_suffix(".blade.php")
                    .or_else(|| name.strip_suffix(".php"));
                let Some(stem) = stem else { continue };
                let Ok(rel) = p.strip_prefix(base) else { continue };
                let mut dotted = rel
                    .parent()
                    .map(|pp| {
                        pp.to_string_lossy()
                            .replace(['/', '\\'], ".")
                    })
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("{s}."))
                    .unwrap_or_default();
                dotted.push_str(stem);
                if out.len() < MAX_VIEW_FILES && !out.contains(&dotted) {
                    out.push(dotted);
                }
            }
        }
    }
    let base = root.join("resources/views");
    let mut out = Vec::new();
    walk(&base, 0, &base, &mut out);
    out.sort();
    out
}

/// `@yield('x')` / `@section('x')` / `@stack('x')` names declared in
/// `resources/views/layouts/**` (the conventional home of layouts; capped).
fn layout_names(root: &Path) -> (Vec<String>, Vec<String>) {
    const MARKERS: &[&str] = &["@yield(", "@section(", "@stack("];
    let base = root.join("resources/views/layouts");
    let mut files = Vec::new();
    walk_php_blade(&base, 0, &mut files);
    let mut sections = Vec::new();
    let mut stacks = Vec::new();
    for f in files.into_iter().take(100) {
        let Ok(md) = std::fs::symlink_metadata(&f) else { continue };
        if !md.is_file() || md.len() > 128 * 1024 {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for marker in MARKERS {
            for (i, _) in text.match_indices(marker) {
                let rest = text[i + marker.len()..].trim_start();
                let Some(q) = rest.chars().next().filter(|c| *c == '\'' || *c == '"')
                else {
                    continue;
                };
                let Some(end) = rest[1..].find(q) else { continue };
                let name = &rest[1..1 + end];
                if name.is_empty() || name.len() > 60 {
                    continue;
                }
                let target = if *marker == "@stack(" {
                    &mut stacks
                } else {
                    &mut sections
                };
                if !target.iter().any(|n: &String| n == name) {
                    target.push(name.to_string());
                }
            }
        }
    }
    (sections, stacks)
}

/// Blade files under `dir` (depth bounded, symlinks skipped).
fn walk_php_blade(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_VIEW_FILES {
        return;
    }
    if depth == 0
        && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink())
    {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.filter_map(Result::ok) {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() {
            walk_php_blade(&p, depth + 1, out);
        } else if ft.is_file()
            && p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".blade.php"))
        {
            out.push(p);
        }
        if out.len() >= MAX_VIEW_FILES {
            return;
        }
    }
}

/// Config keys and env keys from the env/config palette index labels
/// (`"env  KEY  (.env)"`, `"config  app.name  (config/app.php)"`).
fn env_config_keys(root: &Path) -> (Vec<String>, Vec<String>) {
    let mut envs = Vec::new();
    let mut configs = Vec::new();
    for t in crate::env_index::discover(root) {
        if let Some(rest) = t.label.strip_prefix("env  ") {
            if let Some((key, _)) = rest.rsplit_once("  (") {
                envs.push(key.to_string());
            }
        } else if let Some(rest) = t.label.strip_prefix("config  ") {
            if let Some((key, _)) = rest.rsplit_once("  (") {
                configs.push(key.to_string());
            }
        }
    }
    (envs, configs)
}

/// Translation keys from the Go to Translation index labels
/// (`"lang  auth.failed  (resources/lang/en/auth.php)"`).
fn lang_keys(root: &Path) -> Vec<String> {
    crate::lang_index::discover(root)
        .into_iter()
        .filter_map(|t| {
            let rest = t.label.strip_prefix("lang  ")?;
            rest.rsplit_once("  (").map(|(k, _)| k.to_string())
        })
        .collect()
}

fn lists(root: &Path) -> Lists {
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    {
        let guard = cache.lock().unwrap();
        if let Some(l) = guard.as_ref() {
            if l.root == root && l.at.elapsed() < TTL {
                return l.clone();
            }
        }
    }
    let routes = route_names(root);
    let views = view_names(root);
    let (envs, configs) = env_config_keys(root);
    let (sections, stacks) = layout_names(root);
    let langs = lang_keys(root);
    let components = component_names(root);
    let assets = asset_paths(root);
    let gates = ability_names(root);
    let fresh = Lists {
        root: root.to_path_buf(),
        at: Instant::now(),
        routes,
        configs,
        envs,
        views,
        sections,
        stacks,
        langs,
        components,
        assets,
        gates,
    };
    *cache.lock().unwrap() = Some(fresh.clone());
    fresh
}

static PROP_CACHE: OnceLock<
    Mutex<Option<(PathBuf, Option<SystemTime>, Vec<String>)>>,
> = OnceLock::new();

fn prop_item(n: String) -> CompletionItem {
    CompletionItem {
        label: n,
        kind: Some(CompletionItemKind::PROPERTY),
        detail: Some("Livewire property".to_string()),
        ..Default::default()
    }
}

/// Public properties of the Livewire component paired with `file`,
/// re-parsed only when the class file changes.
fn livewire_props(root: &Path, file: Option<&Path>) -> Vec<CompletionItem> {
    let Some(file) = file else { return Vec::new() };
    let Some(class) = crate::livewire_handlers::class_for_view(root, file)
    else {
        return Vec::new();
    };
    let Ok(md) = std::fs::metadata(&class) else { return Vec::new() };
    let mtime = md.modified().ok();
    let cache = PROP_CACHE.get_or_init(|| Mutex::new(None));
    {
        let guard = cache.lock().unwrap();
        if let Some((p, m, props)) = guard.as_ref() {
            if *p == class && *m == mtime {
                return props
                    .iter()
                    .take(MAX_ITEMS)
                    .cloned()
                    .map(prop_item)
                    .collect();
            }
        }
    }
    let Ok(text) = std::fs::read_to_string(&class) else { return Vec::new() };
    let props = crate::livewire_handlers::public_props(&text);
    *cache.lock().unwrap() = Some((class, mtime, props.clone()));
    props.into_iter().take(MAX_ITEMS).map(prop_item).collect()
}

/// Variables a Blade view defines above the cursor: `$name = ...`
/// assignments, `@foreach` / `@forelse` loop variables, `@props([...])`
/// entries and `@inject('name', ...)` names. The compiled view is one PHP
/// function scope, so anything assigned above the cursor is in scope.
/// Static text scan only; at most the last 256 KiB before the cursor.
pub fn blade_variables(
    text_before: &str,
    line_before: &str,
) -> Vec<CompletionItem> {
    // Only when the line ends inside a `$<partial>` token.
    let bytes = line_before.as_bytes();
    let mut i = bytes.len();
    while i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_')
    {
        i -= 1;
    }
    if i == 0 || bytes[i - 1] != b'$' {
        return Vec::new();
    }
    if i >= 2 {
        let p = bytes[i - 2];
        if p.is_ascii_alphanumeric() || p == b'_' || p == b'$' {
            return Vec::new();
        }
    }

    // At most the last 256 KiB.
    let mut start = text_before.len().saturating_sub(262_144);
    while !text_before.is_char_boundary(start) {
        start += 1;
    }
    // Strip Blade comments so they cannot define variables.
    let mut cleaned = String::with_capacity(text_before.len() - start);
    let mut rest = &text_before[start..];
    while let Some(o) = rest.find("{{--") {
        cleaned.push_str(&rest[..o]);
        rest = match rest[o..].find("--}}") {
            Some(c) => &rest[o + c + 4..],
            None => "",
        };
    }
    cleaned.push_str(rest);
    let text = cleaned.as_str();

    let mut vars: Vec<String> = Vec::new();
    let mut push = |name: &str, vars: &mut Vec<String>| {
        if !name.is_empty()
            && name.len() <= 60
            && !vars.iter().any(|v| v == name)
        {
            vars.push(name.to_string());
        }
    };

    // `$name` occurrences that are definitions: followed by a single `=`,
    // or preceded by `as` (loop variables) or `=>` (foreach key => value).
    let tb = text.as_bytes();
    let mut at = 0;
    while at < tb.len() {
        if tb[at] != b'$' {
            at += 1;
            continue;
        }
        let ns = at + 1;
        let mut ne = ns;
        while ne < tb.len()
            && (tb[ne].is_ascii_alphanumeric() || tb[ne] == b'_')
        {
            ne += 1;
        }
        let name = &text[ns..ne];
        at = ne.max(ns);
        if name.is_empty() || name.as_bytes()[0].is_ascii_digit() {
            continue;
        }
        let mut fw = ne;
        while fw < tb.len() && (tb[fw] == b' ' || tb[fw] == b'\t') {
            fw += 1;
        }
        let assigns = fw < tb.len()
            && tb[fw] == b'='
            && tb.get(fw + 1) != Some(&b'=')
            && tb.get(fw + 1) != Some(&b'>');
        // Look back from `$` over spaces, then at the last three chars
        // (char-aware: the bytes before may be multibyte).
        let mut bw = ns - 1;
        while bw > 0 && (tb[bw - 1] == b' ' || tb[bw - 1] == b'\t') {
            bw -= 1;
        }
        let tail: Vec<char> = text[..bw].chars().rev().take(3).collect();
        let after_as = tail.len() >= 2
            && tail[0].eq_ignore_ascii_case(&'s')
            && tail[1].eq_ignore_ascii_case(&'a')
            && tail
                .get(2)
                .map_or(true, |c| !(c.is_ascii_alphanumeric() || *c == '_'));
        let after_fat =
            tail.len() >= 2 && tail[0] == '>' && tail[1] == '=';
        if assigns || after_as || after_fat {
            push(name, &mut vars);
        }
    }

    // @props(['disabled' => ..., 'type']) keys and bare entries. A
    // quoted string is a prop name only when followed by `=>` (key) or
    // by `,` / `]` / `)` (bare entry) — and never right after `=>`
    // (that is a default value, not a name).
    let mut p = 0;
    while let Some(o) = text[p..].find("@props") {
        let o = p + o;
        let after = &text[o + 6..];
        let Some(op) = after.find('(') else { break };
        let Some(cp) = after[op..].find(')') else { break };
        let inner = &after[op + 1..op + cp];
        let ib = inner.as_bytes();
        let mut q = 0;
        while q < ib.len() {
            let quote = ib[q];
            if quote != b'\'' && quote != b'"' {
                q += 1;
                continue;
            }
            let Some(e) = inner[q + 1..].find(quote as char).map(|e| q + 1 + e)
            else {
                break;
            };
            // Skip whitespace after the closing quote.
            let mut n = e + 1;
            while n < ib.len() && (ib[n] == b' ' || ib[n] == b'\t') {
                n += 1;
            }
            // Non-whitespace before the opening quote.
            let mut b = q;
            while b > 0 && (ib[b - 1] == b' ' || ib[b - 1] == b'\t') {
                b -= 1;
            }
            let is_value = b >= 2 && ib[b - 2] == b'=' && ib[b - 1] == b'>';
            let is_key = ib.get(n) == Some(&b'=')
                && ib.get(n + 1) == Some(&b'>');
            let is_bare = matches!(ib.get(n), Some(b',') | Some(b']') | Some(b')'));
            if !is_value && (is_key || is_bare) {
                push(&inner[q + 1..e], &mut vars);
            }
            q = e + 1;
        }
        p = o + 6 + op + cp;
    }

    // @inject('name', 'Service') first argument.
    let mut p = 0;
    while let Some(o) = text[p..].find("@inject") {
        let o = p + o;
        let after = &text[o + 7..];
        let Some(q1) = after.find(['\'', '"']) else { break };
        let quote = after.as_bytes()[q1];
        let Some(q2) = after[q1 + 1..].find(quote as char).map(|e| q1 + 1 + e)
        else {
            break;
        };
        push(&after[q1 + 1..q2], &mut vars);
        p = o + 7 + q2;
    }

    let builtins = builtin_blade_variables(text, &vars);
    vars.into_iter()
        .take(100)
        .map(|n| CompletionItem {
            label: n,
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("Blade variable".to_string()),
            ..Default::default()
        })
        .chain(builtins)
        .collect()
}

/// Blade's own variables that are in scope without any assignment the
/// scanner could see: `$errors` is shared with every view, `$loop` exists
/// inside an unclosed `@foreach`/`@forelse`, `$message` inside `@error`,
/// and `$slot` / `$attributes` in a component view (one with `@props`).
fn builtin_blade_variables(
    text: &str,
    vars: &[String],
) -> Vec<CompletionItem> {
    let mut out: Vec<CompletionItem> = Vec::new();
    let mut push = |name: &str, detail: &str| {
        if !vars.iter().any(|v| v == name)
            && !out.iter().any(|i| i.label == name)
        {
            out.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some(detail.to_string()),
                ..Default::default()
            });
        }
    };
    push("errors", "Blade: shared ViewErrorBag of validation errors");
    let open_loops = (text.matches("@foreach").count()
        + text.matches("@forelse").count())
    .saturating_sub(
        text.matches("@endforeach").count() + text.matches("@endforelse").count(),
    );
    if open_loops > 0 {
        push(
            "loop",
            "Blade: current iteration ($loop->index, ->iteration, ->first, ->last, ->remaining, ->count, ->depth)",
        );
    }
    let open_error = text
        .matches("@error")
        .count()
        .saturating_sub(text.matches("@enderror").count());
    if open_error > 0 {
        push("message", "Blade: the field's validation message inside @error");
    }
    if text.contains("@props") {
        push("slot", "Blade component: default slot content");
        push(
            "attributes",
            "Blade component: attribute bag ($attributes->merge([...]))",
        );
    }
    out
}

static METHOD_CACHE: std::sync::OnceLock<
    Mutex<Option<(PathBuf, Option<SystemTime>, Vec<String>)>>,
> = std::sync::OnceLock::new();

fn method_item(n: String) -> CompletionItem {
    CompletionItem {
        label: n,
        kind: Some(CompletionItemKind::METHOD),
        detail: Some("Livewire method".to_string()),
        ..Default::default()
    }
}

/// Public methods of the Livewire component paired with `file`,
/// re-parsed only when the class file changes.
fn livewire_methods(root: &Path, file: Option<&Path>) -> Vec<CompletionItem> {
    let Some(file) = file else { return Vec::new() };
    let Some(class) = crate::livewire_handlers::class_for_view(root, file)
    else {
        return Vec::new();
    };
    let Ok(md) = std::fs::metadata(&class) else { return Vec::new() };
    let mtime = md.modified().ok();
    let cache = METHOD_CACHE.get_or_init(|| Mutex::new(None));
    {
        let guard = cache.lock().unwrap();
        if let Some((p, m, methods)) = guard.as_ref() {
            if *p == class && *m == mtime {
                return methods
                    .iter()
                    .take(MAX_ITEMS)
                    .cloned()
                    .map(method_item)
                    .collect();
            }
        }
    }
    let Ok(text) = std::fs::read_to_string(&class) else { return Vec::new() };
    let methods = crate::livewire_handlers::public_methods(&text);
    *cache.lock().unwrap() = Some((class, mtime, methods.clone()));
    methods.into_iter().take(MAX_ITEMS).map(method_item).collect()
}

/// Completion items for the project context at the end of `line_before`.
/// `file` is the edited file; only Livewire completion uses it.
/// Gate abilities: public policy methods under `app/Policies`
/// (reusing the policy index parser) plus `Gate::define('name')` names
/// from `app/Providers`. Static text scan, bounded like the index.
fn ability_names(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if depth > 3 || out.len() >= 500 {
            return;
        }
        if depth == 0
            && std::fs::symlink_metadata(dir)
                .map_or(true, |m| m.file_type().is_symlink())
        {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.filter_map(Result::ok) {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let p = e.path();
            if ft.is_dir() {
                walk(&p, depth + 1, out);
            } else if ft.is_file()
                && p.extension().is_some_and(|x| x == "php")
            {
                out.push(p);
            }
            if out.len() >= 500 {
                return;
            }
        }
    }
    fn read(p: &Path) -> Option<String> {
        let m = std::fs::metadata(p).ok()?;
        if m.len() > (1 << 20) {
            return None;
        }
        std::fs::read_to_string(p).ok()
    }
    let mut names: Vec<String> = Vec::new();
    let mut push = |n: &str, names: &mut Vec<String>| {
        if !n.is_empty() && n.len() <= 100 && !names.iter().any(|x| x == n) {
            names.push(n.to_string());
        }
    };
    let mut files = Vec::new();
    walk(&root.join("app/Policies"), 0, &mut files);
    for f in files.drain(..) {
        let Some(text) = read(&f) else { continue };
        let Some(p) = crate::policy_index::parse_policy(&text) else {
            continue;
        };
        for (a, _, _) in p.abilities {
            push(&a, &mut names);
        }
    }
    walk(&root.join("app/Providers"), 0, &mut files);
    for f in files {
        let Some(text) = read(&f) else { continue };
        for (n, _) in crate::policy_index::parse_gates(&text) {
            push(&n, &mut names);
        }
    }
    names.truncate(2000);
    names
}

/// Files under `public/` as `asset()`-ready paths (`css/app.css`).
/// Symlinks (common for `public/storage`) and hidden directories are
/// skipped; the walk is bounded like the other static scans.
fn asset_paths(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, depth: usize, base: &Path, out: &mut Vec<String>) {
        if depth > MAX_DEPTH || out.len() >= MAX_VIEW_FILES {
            return;
        }
        if depth == 0
            && std::fs::symlink_metadata(dir)
                .map_or(true, |m| m.file_type().is_symlink())
        {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.filter_map(Result::ok) {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let p = e.path();
            let hidden = e
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with('.'));
            if hidden {
                continue;
            }
            if ft.is_dir() {
                walk(&p, depth + 1, base, out);
            } else if ft.is_file() {
                let Ok(rel) = p.strip_prefix(base) else { continue };
                let path = rel.to_string_lossy().replace('\\', "/");
                if out.len() < MAX_VIEW_FILES && !out.contains(&path) {
                    out.push(path);
                }
            }
        }
    }
    let base = root.join("public");
    let mut out = Vec::new();
    walk(&base, 0, &base, &mut out);
    out
}

/// Blade component tag names from `resources/views/components`:
/// `forms/input.blade.php` -> `forms.input`, and an `index.blade.php`
/// names its directory, matching the component resolver in
/// `props_draft.rs`. Symlinks are not followed; bounded walk.
fn component_names(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, depth: usize, base: &Path, out: &mut Vec<String>) {
        if depth > MAX_DEPTH || out.len() >= MAX_VIEW_FILES {
            return;
        }
        if depth == 0
            && std::fs::symlink_metadata(dir)
                .map_or(true, |m| m.file_type().is_symlink())
        {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.filter_map(Result::ok) {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let p = e.path();
            if ft.is_dir() {
                walk(&p, depth + 1, base, out);
            } else if ft.is_file() {
                let Some(name) = p.file_name().and_then(|n| n.to_str())
                else {
                    continue;
                };
                let Some(stem) = name.strip_suffix(".blade.php") else {
                    continue;
                };
                let Ok(rel) = p.strip_prefix(base) else { continue };
                let parent = rel
                    .parent()
                    .map(|pp| pp.to_string_lossy().replace(['/', '\\'], "."))
                    .filter(|s| !s.is_empty());
                let dotted = if stem == "index" {
                    match parent {
                        Some(d) => d,
                        None => continue,
                    }
                } else {
                    match parent {
                        Some(d) => format!("{d}.{stem}"),
                        None => stem.to_string(),
                    }
                };
                if out.len() < MAX_VIEW_FILES && !out.contains(&dotted) {
                    out.push(dotted);
                }
            }
        }
    }
    let base = root.join("resources").join("views").join("components");
    let mut out = Vec::new();
    walk(&base, 0, &base, &mut out);
    out
}

/// `<x-...>` tag completion: only inside a `.blade.php` view.
fn component_tags(root: &Path, file: Option<&Path>) -> Vec<CompletionItem> {
    let is_blade = file
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".blade.php"));
    if !is_blade {
        return Vec::new();
    }
    lists(root)
        .components
        .into_iter()
        .take(MAX_ITEMS)
        .map(|n| CompletionItem {
            label: n,
            kind: Some(CompletionItemKind::CLASS),
            detail: Some("Blade component".to_string()),
            ..Default::default()
        })
        .collect()
}

/// kebab-case of a StudlyCaps class name: `UserProfile` -> `user-profile`.
fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
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

/// Livewire component names: view files under `resources/views/livewire`
/// and classes under `app/Livewire` / `app/Http/Livewire`, dotted and
/// kebab-cased (`Admin/UserProfile.php` -> `admin.user-profile`). Symlinks
/// are not followed; bounded walk.
fn livewire_names(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, depth: usize, base: &Path, ext: &str, out: &mut Vec<String>) {
        if depth > MAX_DEPTH || out.len() >= MAX_VIEW_FILES {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.filter_map(Result::ok) {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let p = e.path();
            if ft.is_dir() {
                walk(&p, depth + 1, base, ext, out);
            } else if ft.is_file() {
                let Some(fname) = p.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let Some(stem) = fname.strip_suffix(ext) else { continue };
                let Ok(rel) = p.strip_prefix(base) else { continue };
                let mut parts: Vec<String> = rel
                    .parent()
                    .map(|pp| {
                        pp.components()
                            .map(|c| c.as_os_str().to_string_lossy().to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                parts.push(stem.to_string());
                let name = parts
                    .iter()
                    .map(|x| kebab(x))
                    .collect::<Vec<_>>()
                    .join(".");
                if !out.contains(&name) && out.len() < MAX_VIEW_FILES {
                    out.push(name);
                }
            }
        }
    }
    let mut out = Vec::new();
    let views = root.join("resources").join("views").join("livewire");
    walk(&views, 0, &views, ".blade.php", &mut out);
    for base in [root.join("app").join("Livewire"), root.join("app").join("Http").join("Livewire")] {
        walk(&base, 0, &base, ".php", &mut out);
    }
    out.sort();
    out
}

/// `<livewire:...>` tag completion: only inside a `.blade.php` view.
fn livewire_tags(root: &Path, file: Option<&Path>) -> Vec<CompletionItem> {
    let is_blade = file
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".blade.php"));
    if !is_blade {
        return Vec::new();
    }
    livewire_names(root)
        .into_iter()
        .take(MAX_ITEMS)
        .map(|n| CompletionItem {
            label: n,
            kind: Some(CompletionItemKind::CLASS),
            detail: Some("Livewire component".to_string()),
            ..Default::default()
        })
        .collect()
}

/// Does the completion context at the end of `line_before_cursor` offer
/// dotted names (routes, config keys, views, sections, stacks, translation
/// keys, Gate abilities)? Used to extend the completion word start over
/// `.`-joined segments so accepting replaces the whole name.
pub fn dotted_name_context(line_before_cursor: &str) -> bool {
    matches!(
        context(line_before_cursor),
        Some(
            Kind::Route
                | Kind::Config
                | Kind::View
                | Kind::Section
                | Kind::Stack
                | Kind::Lang
                | Kind::Gate
                | Kind::Lang
                | Kind::Gate
                | Kind::LivewireName
        )
    )
}

/// Bytes to extend a completion word start backwards over `.`- and
/// `-`-joined segments of a quoted name (`admin.us`,
/// `messages.welcome-back`). Stops at anything else, such as the opening
/// quote. Returns 0 when `before` does not end in a joined segment.
pub fn dotted_word_len(before: &str) -> usize {
    let mut rest = before;
    let mut total = 0;
    while let Some(sep) =
        rest.chars().next_back().filter(|c| matches!(c, '.' | '-'))
    {
        let r = &rest[..rest.len() - sep.len_utf8()];
        let word: usize = r
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .map(char::len_utf8)
            .sum();
        if word == 0 {
            break;
        }
        total += sep.len_utf8() + word;
        rest = &r[..r.len() - word];
    }
    total
}

pub fn items(
    root: &Path,
    file: Option<&Path>,
    line_before: &str,
) -> Vec<CompletionItem> {
    if line_before.len() > 4096 {
        return Vec::new();
    }
    let Some(kind) = context(line_before) else { return Vec::new() };
    if kind == Kind::LivewireModel {
        return livewire_props(root, file);
    }
    if kind == Kind::LivewireAction {
        return livewire_methods(root, file);
    }
    if kind == Kind::ComponentTag {
        return component_tags(root, file);
    }
    if kind == Kind::LivewireTag {
        return livewire_tags(root, file);
    }
    if kind == Kind::LivewireName {
        return livewire_tags(root, file);
    }
    let lists = lists(root);
    let (names, detail) = match kind {
        Kind::Route => (lists.routes, "route name"),
        Kind::Config => (lists.configs, "config key"),
        Kind::Env => (lists.envs, "env key"),
        Kind::View => (lists.views, "Blade view"),
        Kind::Section => (lists.sections, "Blade section"),
        Kind::Stack => (lists.stacks, "Blade stack"),
        Kind::Lang => (lists.langs, "translation key"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireModel => (Vec::new(), "Livewire property"),
        Kind::LivewireAction => (Vec::new(), "Livewire method"),
        // Returned early above; the match must stay exhaustive.
        Kind::ComponentTag => (Vec::new(), "Blade component"),
        // Returned early above; the match must stay exhaustive.
        Kind::LivewireName => (Vec::new(), "Livewire component"),
        Kind::LivewireTag => (Vec::new(), "Livewire component"),
        Kind::Asset => (lists.assets, "asset path"),
        Kind::Gate => (lists.gates, "Gate ability"),
    };
    names
        .into_iter()
        .take(MAX_ITEMS)
        .map(|n| CompletionItem {
            label: n,
            kind: Some(CompletionItemKind::VALUE),
            detail: Some(detail.to_string()),
            ..Default::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
    }

    #[test]
    fn detects_contexts() {
        assert_eq!(context("route('us"), Some(Kind::Route));
        assert_eq!(context("to_route(\"us"), Some(Kind::Route));
        assert_eq!(context("Route::has('adm"), Some(Kind::Route));
        assert_eq!(context("config('app.na"), Some(Kind::Config));
        assert_eq!(context("env('DB_"), Some(Kind::Env));
        assert_eq!(context("view('pages."), Some(Kind::View));
        // Closed string, after the call, unknown function, Collection::has.
        assert_eq!(context("route('users.index')"), None);
        assert_eq!(context("route('users.index')->x"), None);
        assert_eq!(context("foo('x"), None);
        assert_eq!(context("$c->has('x"), None);
        assert_eq!(context(""), None);
        // Blade directives.
        assert_eq!(context("@extends('lay"), Some(Kind::View));
        assert_eq!(context("@include('partials."), Some(Kind::View));
        assert_eq!(context("@section('con"), Some(Kind::Section));
        assert_eq!(context("@yield('con"), Some(Kind::Section));
        assert_eq!(context("@push('scr"), Some(Kind::Stack));
        assert_eq!(context("@stack('mod"), Some(Kind::Stack));
        // Without the @, these are ordinary calls.
        assert_eq!(context("yield('x"), None);
        assert_eq!(context("section('x"), None);
        // Translation keys.
        assert_eq!(context("__('wel"), Some(Kind::Lang));
        assert_eq!(context("__(\"wel"), Some(Kind::Lang));
        assert_eq!(context("trans('mess"), Some(Kind::Lang));
        assert_eq!(context("trans_choice('apple"), Some(Kind::Lang));
        assert_eq!(context("@lang('mess"), Some(Kind::Lang));
        assert_eq!(context("@choice('apple"), Some(Kind::Lang));
        assert_eq!(context("Lang::get('mess"), Some(Kind::Lang));
        assert_eq!(context("Lang::has('mess"), Some(Kind::Lang));
        // Without the @ or Lang::, these are ordinary calls.
        assert_eq!(context("lang('x"), None);
        assert_eq!(context("$c->choice('x"), None);
        assert_eq!(context("Str::get('x"), None);
        // Livewire wire:model, any quote style or modifier chain.
        assert_eq!(
            context("<input wire:model=\"em"),
            Some(Kind::LivewireModel)
        );
        assert_eq!(
            context("<input wire:model.live=\"em"),
            Some(Kind::LivewireModel)
        );
        assert_eq!(
            context("<input wire:model.debounce.300ms='em"),
            Some(Kind::LivewireModel)
        );
        // Closed value, other attributes and Alpine are not model contexts.
        assert_eq!(context("<input wire:model=\"em\""), None);
        assert_eq!(context("<div x-data=\"{ m: 'x' }\">"), None);
        // Livewire event attributes complete methods.
        assert_eq!(
            context("<button wire:click=\"sa"),
            Some(Kind::LivewireAction)
        );
        assert_eq!(
            context("<form wire:submit.prevent=\"sa"),
            Some(Kind::LivewireAction)
        );
        assert_eq!(
            context("<input wire:keydown.enter='sa"),
            Some(Kind::LivewireAction)
        );
        // Non-event attributes and closed values are not method contexts.
        assert_eq!(context("<div wire:loading.cla"), None);
        assert_eq!(context("<div wire:key=\"it"), None);
        assert_eq!(context("<button wire:click=\"sa\""), None);
        assert_eq!(context("<button data-wire:click=\"sa"), None);
        assert_eq!(context("<button x-on:wire:click=\"sa"), None);
    }

    #[test]
    fn offers_project_names() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "routes/web.php", "<?php\nRoute::get('/users', [UserController::class, 'index'])->name('users.index');\nRoute::get('/', fn () => view('home'))->name(\"home\");\n");
        put(r, "config/app.php", "<?php\nreturn [\n    'name' => env('APP_NAME', 'Laravel'),\n];\n");
        put(r, ".env", "APP_NAME=Acme\nDB_HOST=db\n");
        put(r, "resources/views/pages/home.blade.php", "<h1>hi</h1>\n");
        put(r, "resources/views/emails/welcome.php", "<p>w</p>\n");
        put(r, "resources/views/layouts/app.blade.php", "<html>\n@yield('content')\n@yield(\"scripts\")\n@stack('modals')\n@section('sidebar')\n@show\n</html>\n");
        put(r, "resources/lang/en/auth.php", "<?php\nreturn [\n    'failed' => 'These credentials do not match.',\n    'throttle' => [\n        'seconds' => 'Too many.',\n    ],\n];\n");
        put(r, "resources/lang/fr/auth.php", "<?php\nreturn [\n    'failed' => 'Ces identifiants ne correspondent pas.',\n    'bonjour' => 'Bonjour',\n];\n");
        put(r, "resources/lang/en.json", "{\n  \"Welcome back\": \"Welcome back\"\n}\n");

        let labels = |kind_line: &str| {
            items(r, None, kind_line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(labels("route('"), ["users.index", "home"]);
        assert!(labels("config('").contains(&"app.name".to_string()));
        assert_eq!(labels("env('"), ["APP_NAME", "DB_HOST"]);
        assert_eq!(
            labels("view('"),
            ["emails.welcome", "layouts.app", "pages.home"]
        );
        assert!(labels("foo('").is_empty());
        assert_eq!(labels("@section('"), ["content", "scripts", "sidebar"]);
        assert_eq!(labels("@yield('"), ["content", "scripts", "sidebar"]);
        assert_eq!(labels("@stack('"), ["modals"]);
        assert!(labels("@extends('").contains(&"layouts.app".to_string()));
        // Translation keys: nested PHP keys dot-flattened, JSON keys as-is,
        // a key in several locales listed once, fr-only keys included.
        let langs = labels("__('");
        assert!(langs.contains(&"auth.failed".to_string()));
        assert!(langs.contains(&"auth.throttle.seconds".to_string()));
        assert!(langs.contains(&"Welcome back".to_string()));
        assert!(langs.contains(&"auth.bonjour".to_string()));
        assert_eq!(
            langs.iter().filter(|l| *l == "auth.failed").count(),
            1
        );
        assert_eq!(labels("trans('"), langs);
        assert_eq!(labels("@lang('"), langs);
    }

    #[test]
    fn offers_livewire_properties() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/livewire/admin/user-profile.blade.php", "<input wire:model=\"email\">\n");
        put(r, "app/Livewire/Admin/UserProfile.php", "<?php\nclass UserProfile extends \\Livewire\\Component {\n    public $email = '';\n    public ?string $name;\n    public array $items = [];\n    public static $shared;\n    protected $secret;\n    public function save() {}\n}\n");
        put(r, "resources/views/livewire/counter.blade.php", "<input wire:model=\"count\">\n");
        put(r, "app/Http/Livewire/Counter.php", "<?php\nclass Counter extends \\Livewire\\Component {\n    public $count = 0;\n}\n");
        let labels = |view: &Path, line: &str| {
            items(r, Some(view), line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let v3 = r.join("resources/views/livewire/admin/user-profile.blade.php");
        let got = labels(&v3, "<input wire:model=\"");
        assert!(got.contains(&"email".to_string()));
        assert!(got.contains(&"name".to_string()));
        assert!(got.contains(&"items".to_string()));
        assert!(!got.contains(&"shared".to_string()));
        assert!(!got.contains(&"secret".to_string()));
        assert!(!got.contains(&"save".to_string()));
        // Livewire v2 layout.
        let v2 = r.join("resources/views/livewire/counter.blade.php");
        assert_eq!(labels(&v2, "<input wire:model=\"co"), ["count"]);
        // A view outside resources/views/livewire has no paired class.
        put(r, "resources/views/pages/home.blade.php", "<input>\n");
        let plain = r.join("resources/views/pages/home.blade.php");
        assert!(labels(&plain, "<input wire:model=\"").is_empty());
        // Without a file there is nothing to pair.
        assert!(items(r, None, "<input wire:model=\"").is_empty());
    }

    #[test]
    fn offers_livewire_methods() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/livewire/user-profile.blade.php", "<button wire:click=\"save\">\n");
        put(r, "app/Livewire/UserProfile.php", "<?php\nclass UserProfile extends \\Livewire\\Component {\n    public $email = '';\n    public function save() {}\n    public function delete() {}\n    private function helper() {}\n    protected function boot() {}\n    public static function make() {}\n}\n");
        let view = r.join("resources/views/livewire/user-profile.blade.php");
        let labels = |line: &str| {
            items(r, Some(view.as_path()), line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>()
        };
        let methods = labels("<button wire:click=\"");
        assert!(methods.contains(&"save".to_string()));
        assert!(methods.contains(&"delete".to_string()));
        // Private, protected and static members are excluded.
        assert!(!methods.contains(&"helper".to_string()));
        assert!(!methods.contains(&"boot".to_string()));
        assert!(!methods.contains(&"make".to_string()));
        // Properties are not methods and methods are not properties.
        assert!(!methods.contains(&"email".to_string()));
        let props = labels("<input wire:model=\"");
        assert!(props.contains(&"email".to_string()));
        assert!(!props.contains(&"save".to_string()));
    }
    fn labels(items: Vec<CompletionItem>) -> Vec<String> {
        items.into_iter().map(|i| i.label).collect()
    }

    const BLADE_VIEW: &str = r#"@props(['disabled' => false, 'type' => 'submit', 'flat'])
@inject('metrics', 'App\Services\Metrics')
@php
    $title = 'Hello';
    $count = $metrics->total();
    $bad == $title;
    $pair = ['k' => 1];
@endphp
@foreach($items as $idx => $item)
    <li>{{ $item->name }} {{ $idx }}</li>
@endforeach
@forelse($users as $user)
    <p>{{ $user->email }}</p>
@endforelse
{{-- $ghost = 1; @inject('sneaky', 'X') --}}
@if($flag)
    {{ $x }}
@endif
"#;

    #[test]
    fn offers_gate_abilities() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Policies/PostPolicy.php", "<?php
final class PostPolicy {
    public function before() {}
    public function viewAny(User $user) {}
    public function update(User $user, Post $post) {}
    protected function hidden() {}
}
");
        put(r, "app/Providers/AuthServiceProvider.php", "<?php
Gate::define('sell-vinyl', [ShopGate::class, 'sell']);
Gate::define(\"buy-cd\", 'shop.buy');
");
        let mut labels = items(r, None, "@can('")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        labels.sort();
        assert_eq!(
            labels,
            [
                "buy-cd".to_string(),
                "sell-vinyl".to_string(),
                "update".to_string(),
                "viewAny".to_string(),
            ]
        );
        // The whole family routes to the same kind.
        for line in [
            "@cannot('",
            "@canany(['",
            "$user->can('",
            "$user?->cannot('",
            "$this->authorize('",
            "Gate::allows('",
            "Gate::denies('",
            "Gate::check('",
        ] {
            assert_eq!(context(line), Some(Kind::Gate), "{line}");
        }
        // Bare `can(` is not a Gate context.
        assert_eq!(context("can('"), None);
        assert_eq!(context("Str::of('"), None);
    }

    #[test]
    fn offers_asset_paths() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "public/css/app.css", "body {}");
        put(r, "public/js/app.js", "");
        put(r, "public/images/logo dark.png", "");
        put(r, "public/.hidden/x.css", "");
        // Missing public/ -> no asset items, other kinds unaffected.
        let labels = |line: &str| {
            let mut v = items(r, None, line)
                .into_iter()
                .map(|i| i.label)
                .collect::<Vec<_>>();
            v.sort();
            v
        };
        assert_eq!(
            labels("asset('"),
            [
                "css/app.css".to_string(),
                "images/logo dark.png".to_string(),
                "js/app.js".to_string(),
            ]
        );
        assert_eq!(labels("secure_asset('"), labels("asset('"));
        assert_eq!(labels("mix('"), labels("asset('"));
        assert!(!labels("asset('").iter().any(|l| l.starts_with(".hidden")));
        let e = tempfile::tempdir().unwrap();
        assert!(items(e.path(), None, "asset('").is_empty());
    }

    #[test]
    fn component_tag_context() {
        assert_eq!(context("<x-"), Some(Kind::ComponentTag));
        assert_eq!(context("<x-al"), Some(Kind::ComponentTag));
        assert_eq!(context(" <x-forms.in"), Some(Kind::ComponentTag));
        assert_eq!(context("route(\"x\") <x-btn"), Some(Kind::ComponentTag));
        // Multibyte chars before the tag must not break the tail scan.
        assert_eq!(context("\u{e9}<x-fo"), Some(Kind::ComponentTag));
        assert_eq!(context("\u{65e5}<x-"), Some(Kind::ComponentTag));
        // Inside an attribute value, after a name char, slots, closed
        // tag name, CSS class values, bare `<x`.
        assert_eq!(context("<x-alert type=\"x-"), None);
        assert_eq!(context("a<x-b"), None);
        assert_eq!(context("x_<x-a"), None);
        assert_eq!(context("<x-slot"), None);
        assert_eq!(context("<x-slot:na"), None);
        assert_eq!(context("<x-alert "), None);
        assert_eq!(context("<div class=\"x-foo"), None);
        assert_eq!(context("<x"), None);
    }

    #[test]
    fn dotted_name_context_and_word_len() {
        assert!(dotted_name_context("route('admin.us"));
        assert!(dotted_name_context("config('app."));
        assert!(dotted_name_context("view('pages.ho"));
        assert!(dotted_name_context("  @include('partials.he"));
        assert!(dotted_name_context("@extends('layouts.ap"));
        assert!(dotted_name_context("__('messages.we"));
        assert!(dotted_name_context("$user->can('edit-po"));
        assert!(dotted_name_context("@livewire('admin.us"));
        assert!(!dotted_name_context("document.getE"));
        assert!(!dotted_name_context("$this->foo->b"));
        assert!(!dotted_name_context("env('APP_"));
        assert_eq!(dotted_word_len("route('admin."), 6);
        assert_eq!(dotted_word_len("config('app."), 4);
        assert_eq!(dotted_word_len("__('messages.welcome-"), 17);
        assert_eq!(dotted_word_len("route('"), 0);
        assert_eq!(dotted_word_len("'"), 0);
        assert_eq!(dotted_word_len(""), 0);
    }

    #[test]
    fn offers_livewire_directive_names() {
        assert_eq!(context("@livewire('"), Some(Kind::LivewireName));
        assert_eq!(context("  @livewire('admin.us"), Some(Kind::LivewireName));
        // Not the directive: no @, so no completion.
        assert_eq!(context("livewire('admin.us"), None);
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Livewire/Admin/UserProfile.php", "");
        put(r, "app/Livewire/Counter.php", "");
        put(r, "resources/views/pages/home.blade.php", "");
        let view = r.join("resources/views/pages/home.blade.php");
        let labels = items(r, Some(&view), "@livewire('")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        assert!(labels.contains(&"admin.user-profile".to_string()));
        assert!(labels.contains(&"counter".to_string()));
    }

    #[test]
    fn offers_livewire_tags() {
        assert_eq!(context("<livewire:"), Some(Kind::LivewireTag));
        assert_eq!(context("  <livewire:admin.us"), Some(Kind::LivewireTag));
        assert_eq!(context("<livewire:counter :n=\"1\""), None);
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "app/Livewire/Admin/UserProfile.php", "");
        put(r, "app/Livewire/Counter.php", "");
        put(r, "resources/views/livewire/only-view.blade.php", "");
        put(r, "resources/views/pages/home.blade.php", "");
        let view = r.join("resources/views/pages/home.blade.php");
        let labels = items(r, Some(&view), "<livewire:")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        assert!(labels.contains(&"admin.user-profile".to_string()));
        assert!(labels.contains(&"counter".to_string()));
        assert!(labels.contains(&"only-view".to_string()));
        let plain = r.join("routes/web.php");
        put(r, "routes/web.php", "<?php");
        assert!(items(r, Some(&plain), "<livewire:").is_empty());
    }

    #[test]
    fn offers_component_tags() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/components/alert.blade.php", "");
        put(r, "resources/views/components/forms/input.blade.php", "");
        put(r, "resources/views/components/card/index.blade.php", "");
        put(r, "resources/views/components/index.blade.php", "");
        put(r, "resources/views/components/plain.php", "");
        put(r, "resources/views/pages/home.blade.php", "");
        let view = r.join("resources/views/pages/home.blade.php");
        let labels = items(r, Some(&view), "<x-")
            .into_iter()
            .map(|i| i.label)
            .collect::<Vec<_>>();
        assert!(labels.contains(&"alert".to_string()));
        assert!(labels.contains(&"forms.input".to_string()));
        // An index.blade.php names its directory; the top-level index
        // names nothing; non-Blade files are not components.
        assert!(labels.contains(&"card".to_string()));
        assert!(!labels.contains(&"index".to_string()));
        assert!(!labels.contains(&"plain".to_string()));
        let item = items(r, Some(&view), "<x-al").into_iter().next().unwrap();
        assert_eq!(item.kind, Some(CompletionItemKind::CLASS));
        // Gate: component tags only complete inside a .blade.php view.
        let plain = r.join("routes/web.php");
        put(r, "routes/web.php", "<?php");
        assert!(items(r, Some(&plain), "<x-").is_empty());
        assert!(items(r, None, "<x-").is_empty());
    }

    #[test]
    fn blade_variables_collects_view_scope() {
        let v = labels(blade_variables(BLADE_VIEW, "    {{ $fo"));
        assert_eq!(
            v,
            [
                "title", "count", "pair", "idx", "item", "user", "disabled",
                "type", "flat", "metrics", "errors", "slot", "attributes"
            ]
        );
        let item = blade_variables(BLADE_VIEW, "{{ $fo")
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(item.kind, Some(CompletionItemKind::VARIABLE));
    }

    #[test]
    fn blade_variables_gate() {
        // Only inside a `$<partial>` token at the end of the line.
        assert!(blade_variables(BLADE_VIEW, "plain text").is_empty());
        assert!(blade_variables(BLADE_VIEW, "{{ $x->fo").is_empty());
        assert!(blade_variables("<?php $$x = 1;", "$$").is_empty());
        assert!(!blade_variables(BLADE_VIEW, "{{ $").is_empty());
    }

    #[test]
    fn blade_variables_excludes_non_definitions() {
        // No view variable is defined; only the always-shared $errors shows.
        assert_eq!(labels(blade_variables("<?php $x += 1;", "$")), ["errors"]);
        assert_eq!(
            labels(blade_variables("<?php $x == 1;", "$x == 1; $")),
            ["errors"]
        );
        assert_eq!(
            labels(blade_variables("{{ $this->foo }}", "{{ $")),
            ["errors"]
        );
        assert_eq!(
            labels(blade_variables("<?php $1abc = 1; $ok2 = 2;", "$")),
            ["ok2", "errors"]
        );
        // Values in @props are not prop names.
        assert_eq!(
            labels(blade_variables(
                "@props([\"tone\" => 'info', \"size\"])",
                "$"
            )),
            ["tone", "size", "errors", "slot", "attributes"]
        );
    }

    #[test]
    fn blade_variables_adds_builtin_scope() {
        // $loop inside an unclosed @foreach, not after @endforeach.
        let t = "@foreach ($items as $i)\n    {{ $lo";
        let v = labels(blade_variables(t, t));
        assert!(v.contains(&"loop".to_string()));
        let t = "@foreach ($items as $i) @endforeach {{ $lo";
        assert!(!labels(blade_variables(t, t)).contains(&"loop".to_string()));
        let t = "@forelse ($items as $i)\n{{ $lo";
        assert!(labels(blade_variables(t, t)).contains(&"loop".to_string()));
        // $message only inside @error.
        let t = "@error('title') {{ $me";
        assert!(labels(blade_variables(t, t)).contains(&"message".to_string()));
        let t = "@error('title') @enderror {{ $me";
        assert!(!labels(blade_variables(t, t)).contains(&"message".to_string()));
        // $errors is always shared; a view-defined $errors wins (no dup).
        let t = "@php($errors = []) @endphp {{ $er";
        let v = labels(blade_variables(t, t));
        assert_eq!(v.iter().filter(|x| *x == "errors").count(), 1);
        // $slot / $attributes only in a component view.
        assert!(!labels(blade_variables("{{ $sl", "{{ $sl"))
            .contains(&"slot".to_string()));
    }

    #[test]
    fn blade_variables_non_ascii_safe() {
        let mut fuzz = String::new();
        let pieces = ["ab", "$", "\u{e9}", "\u{65e5}", " \t", "=", "<>", "@", "'\"", "{{}}"];
        let mut n = 0usize;
        while fuzz.len() < 4000 {
            fuzz.push_str(pieces[n % pieces.len()]);
            n += 1;
        }
        let _ = blade_variables(&fuzz, "$a");
        let _ = blade_variables(&fuzz, "\u{e9}$x");
        let _ = blade_variables(BLADE_VIEW, "{{ $n\u{e9}");
    }
}
