//! Project-aware completion items: inside `route('...')` offer the app's
//! named routes, inside `config('...')` its config keys, inside `env('...')`
//! the env keys, inside `view('...')` its Blade view names, inside
//! `__('...')` its translation keys, and inside `wire:model="..."` in a
//! Livewire view the paired component's public properties. Same static
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
}

/// The context at the end of `line` (the text before the cursor): the last
/// `word(` whose argument string is still open. Quote escapes are not read;
/// `route('it\'s')` is rare and simply falls out of context.
pub fn context(line: &str) -> Option<Kind> {
    // Livewire `wire:model(.modifier)*="..."`: an open attribute value.
    // No parenthesis involved, so this runs before the call-argument scan.
    if let Some(at) = line.rfind("wire:model") {
        let after = &line[at + "wire:model".len()..];
        let attr_len = after
            .find(|c: char| {
                !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            })
            .unwrap_or(after.len());
        let rest = &after[attr_len..];
        let quoted = rest
            .strip_prefix('=')
            .and_then(|r| r.chars().next().map(|q| (q, &r[q.len_utf8()..])));
        if let Some((q, value)) = quoted {
            if (q == '"' || q == '\'') && !value.contains(q) {
                return Some(Kind::LivewireModel);
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
        // Blade directives take view names too.
        "extends" | "include" | "includeIf" | "includeWhen" | "includeFirst"
        | "each" if blade => Kind::View,
        "section" | "yield" if blade => Kind::Section,
        "push" | "pushOnce" | "stack" if blade => Kind::Stack,
        "__" | "trans" | "trans_choice" => Kind::Lang,
        "lang" | "choice" if blade => Kind::Lang,
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

/// Completion items for the project context at the end of `line_before`.
/// `file` is the edited file; only Livewire completion uses it.
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
        assert_eq!(context("<input wire:click=\"sa"), None);
        assert_eq!(context("<div x-data=\"{ m: 'x' }\">"), None);
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
}
