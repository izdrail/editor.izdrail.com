//! WordPress hook navigation without a language server.
//!
//! - On a callback in `add_action('init', 'my_cb')`, `add_filter('x', [$this, 'run'])`,
//!   `add_shortcode('tag', 'cb')` or `register_activation_hook(__FILE__, 'cb')`:
//!   go to `function my_cb(` / `function run(`.
//! - On a hook name in `add_action('my_hook', ...)`: go to the first
//!   `do_action('my_hook'` / `apply_filters('my_hook'` that fires it.
//! - On a hook name in `do_action('my_hook')` / `apply_filters('my_hook', ...)`:
//!   go to the first `add_action` / `add_filter` that registers it.
//! - On a shortcode tag in `do_shortcode('[gallery id="1"]')`, `shortcode_exists('gallery')`
//!   or `has_shortcode($c, 'gallery')`: go to its `add_shortcode('gallery', ...)` call.
//!
//! Textual only: the current file is searched first, then PHP files under the
//! workspace root (skipping `.git`, `node_modules`, `vendor`; at most 4000
//! files, 1 MB each, 400 ms). Unsaved edits are not seen. Nothing is executed.
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_FILES: usize = 4000;
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const BUDGET: Duration = Duration::from_millis(400);
const SKIP_DIRS: &[&str] = &[".git", "node_modules", "vendor", ".svn", ".hg"];

const REGISTER: &[&str] = &["add_action", "add_filter"];
const FIRE: &[&str] = &[
    "do_action", "apply_filters", "do_action_ref_array", "apply_filters_ref_array",
];
/// Calls whose second argument is a callback.
const CALLBACK_CALLERS: &[&str] = &[
    "add_action", "add_filter", "add_shortcode", "register_activation_hook",
    "register_deactivation_hook", "register_uninstall_hook",
];

#[derive(Debug, PartialEq, Eq)]
enum Ref {
    Callback(String),
    HookRegistered(String),
    HookFired(String),
    /// A shortcode tag used in `do_shortcode('[tag]')` or named in
    /// `shortcode_exists('tag')`; goes to its `add_shortcode('tag', ...)`.
    Shortcode(String),
    /// A post type, taxonomy, script/style handle or option name used somewhere;
    /// goes to the first call in the list that registers it.
    Registered(&'static [&'static str], String),
}

const SHORTCODE_NAME_CALLS: &[&str] = &["shortcode_exists", "remove_shortcode"];

/// The shortcode tag at byte `rel` of string `s`: `[tag a="1"]`, `[/tag]`.
fn shortcode_tag_at(s: &str, rel: usize) -> Option<String> {
    let mut rel = rel.min(s.len());
    while !s.is_char_boundary(rel) {
        rel -= 1;
    }
    let open = s[..rel].rfind('[')?;
    // The cursor must still be inside the tag name (no `]` or space between).
    let after = &s[open + 1..];
    let after = after.strip_prefix('/').unwrap_or(after);
    let name: String = after
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    let name_start = s.len() - after.len();
    (!name.is_empty() && rel >= name_start && rel <= name_start + name.len()).then_some(name)
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The quoted string containing byte `col`: `(contents, text_before_quote)`.
fn string_at(line: &str, col: usize) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\'' || bytes[i] == b'"' {
            let q = bytes[i] as char;
            let start = i + 1;
            let end = start + line[start..].find(q)?;
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

/// The text after `name(` for the last call of `name` in `before`.
fn args_after<'a>(before: &'a str, name: &str) -> Option<&'a str> {
    let mut best = None;
    let mut from = 0;
    while let Some(i) = before[from..].find(name).map(|i| i + from) {
        let prev_ok = before[..i].chars().next_back().is_none_or(|c| !is_ident(c) && c != '$' && c != '>');
        let rest = before[i + name.len()..].trim_start();
        if prev_ok {
            if let Some(args) = rest.strip_prefix('(') {
                best = Some(args);
            }
        }
        from = i + name.len();
    }
    best
}

/// `'first_arg',` -> the rest after it (for string literals or `__FILE__`).
fn skip_first_arg(args: &str) -> Option<&str> {
    let a = args.trim_start();
    let rest = if let Some(r) = a.strip_prefix("__FILE__") {
        r
    } else {
        let q = a.chars().next().filter(|c| *c == '\'' || *c == '"')?;
        let body = &a[1..];
        &body[body.find(q)? + 1..]
    };
    rest.trim_start().strip_prefix(',').map(str::trim_start)
}

/// Is `rest` the position of a callback: nothing, or `[`/`array(` plus one
/// receiver token and a comma (`$this,`, `__CLASS__,`, `'Cls',`, `Cls::class,`)?
/// Returns whether it is the array form.
fn callback_position(rest: &str) -> Option<bool> {
    let r = rest.trim();
    if r.is_empty() {
        return Some(false);
    }
    let inner = r.strip_prefix('[').or_else(|| r.strip_prefix("array("))?.trim();
    let recv = inner.strip_suffix(',')?.trim();
    let ok = !recv.is_empty()
        && recv.len() < 80
        && recv.chars().all(|c| is_ident(c) || matches!(c, '$' | '\\' | ':' | '\'' | '"'));
    ok.then_some(true)
}

/// `$content,` -> rest after the first argument when it is a plain
/// variable or call-free expression (used for `has_shortcode($content, 'x')`).
fn skip_first_arg_expr(args: &str) -> Option<&str> {
    let (first, rest) = args.split_once(',')?;
    let f = first.trim();
    let ok = !f.is_empty()
        && f.len() < 80
        && f.chars().all(|c| is_ident(c) || matches!(c, '$' | '>' | '-' | ' '));
    (ok && rest.trim().is_empty()).then_some(rest)
}

/// Usage -> registration for names that are declared once and used by string:
/// `(call whose first argument is the name, array key, registering calls)`.
const USES: &[(&[&str], &str, &[&str])] = &[
    (&["is_singular", "is_post_type_archive", "get_post_type_object", "post_type_exists"], "post_type", &["register_post_type"]),
    (&["is_tax", "get_taxonomy", "taxonomy_exists"], "taxonomy", &["register_taxonomy"]),
    (&["wp_enqueue_script", "wp_dequeue_script", "wp_deregister_script", "wp_script_is"], "", &["wp_register_script"]),
    (&["wp_enqueue_style", "wp_dequeue_style", "wp_deregister_style", "wp_style_is"], "", &["wp_register_style"]),
    (&["get_option", "delete_option"], "", &["add_option", "update_option"]),
];

/// `'post_type' => 'book'`: the array key before a string value.
fn array_key_before(before: &str) -> Option<&str> {
    let t = before.trim_end().strip_suffix("=>")?.trim_end();
    let t = t.strip_suffix(['\'', '"'])?;
    let start = t.char_indices().rev().find(|(_, c)| !is_ident(*c)).map_or(0, |(i, c)| i + c.len_utf8());
    Some(&t[start..])
}

fn registered_name(s: &str, before: &str) -> Option<Ref> {
    if s.len() > 60 || !s.chars().all(|c| is_ident(c) || matches!(c, '-' | '.')) {
        return None;
    }
    for (calls, key, registers) in USES {
        let by_call = calls.iter().any(|c| args_after(before, c).is_some_and(|a| a.trim().is_empty()));
        let by_key = !key.is_empty() && array_key_before(before) == Some(*key);
        if by_call || by_key {
            return Some(Ref::Registered(registers, s.to_string()));
        }
    }
    None
}

fn classify(line: &str, col: usize) -> Option<Ref> {
    let (s, before) = string_at(line, col.min(line.len()))?;
    if s.is_empty() || s.len() > 200 {
        return None;
    }
    // Shortcode: a `[tag]` inside do_shortcode(), or a tag name argument.
    if args_after(before, "do_shortcode").is_some_and(|a| a.trim().is_empty()) {
        let rel = col.saturating_sub(before.len() + 1);
        return shortcode_tag_at(s, rel).map(Ref::Shortcode);
    }
    for c in SHORTCODE_NAME_CALLS {
        if args_after(before, c).is_some_and(|a| a.trim().is_empty()) {
            let ok = s.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
            return ok.then(|| Ref::Shortcode(s.to_string()));
        }
    }
    if let Some(a) = args_after(before, "has_shortcode") {
        if skip_first_arg_expr(a).is_some() {
            let ok = s.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
            return ok.then(|| Ref::Shortcode(s.to_string()));
        }
    }
    if let Some(r) = registered_name(s, before) {
        return Some(r);
    }
    // Hook name: first argument of a registering or firing call.
    for (calls, fired) in [(REGISTER, false), (FIRE, true)] {
        for c in calls {
            if args_after(before, c).is_some_and(|a| a.trim().is_empty()) {
                let hook = s.to_string();
                return Some(if fired { Ref::HookFired(hook) } else { Ref::HookRegistered(hook) });
            }
        }
    }
    // Callback: second argument.
    for c in CALLBACK_CALLERS {
        let Some(args) = args_after(before, c) else { continue };
        let Some(rest) = skip_first_arg(args) else { continue };
        let array = callback_position(rest)?;
        let name = if array {
            s
        } else {
            s.rsplit("::").next().unwrap_or(s)
        };
        let name = name.trim_start_matches('\\');
        let valid = !name.is_empty() && name.chars().all(|ch| is_ident(ch) || ch == '\\');
        return valid.then(|| Ref::Callback(name.rsplit('\\').next().unwrap_or(name).to_string()));
    }
    None
}

/// Zero-based line of `function name(` in `text`.
fn find_function(text: &str, name: &str) -> Option<usize> {
    let pat = format!("function {name}");
    let pat_ref = format!("function &{name}");
    text.lines().position(|l| {
        [&pat, &pat_ref].iter().any(|p| {
            l.match_indices(p.as_str()).any(|(i, _)| {
                let before_ok = l[..i].chars().next_back().is_none_or(|c| !is_ident(c));
                let after = l[i + p.len()..].chars().next();
                before_ok && after.is_some_and(|c| c == '(' || c.is_whitespace())
            })
        })
    })
}

/// Zero-based line of the first `call('hook'` for any call in `calls`.
fn find_hook_call(text: &str, calls: &[&str], hook: &str) -> Option<usize> {
    if !text.contains(hook) {
        return None;
    }
    text.lines().position(|l| {
        calls.iter().any(|c| {
            l.match_indices(c).any(|(i, _)| {
                let ok_before = l[..i].chars().next_back().is_none_or(|ch| !is_ident(ch) && ch != '$' && ch != '>');
                let rest = l[i + c.len()..].trim_start();
                let Some(rest) = rest.strip_prefix('(') else { return false };
                let rest = rest.trim_start();
                let Some(q) = rest.chars().next().filter(|q| *q == '\'' || *q == '"') else {
                    return false;
                };
                ok_before
                    && rest[1..].strip_prefix(hook).is_some_and(|r| r.starts_with(q))
            })
        })
    })
}

fn php_files(root: &Path, first: &Path) -> Vec<PathBuf> {
    let start = Instant::now();
    let mut out = vec![first.to_path_buf()];
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if out.len() >= MAX_FILES || start.elapsed() > BUDGET {
            break;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                let n = e.file_name();
                if !SKIP_DIRS.iter().any(|s| n == *s) {
                    stack.push(p);
                }
            } else if ft.is_file()
                && p.extension().is_some_and(|x| x == "php")
                && p != first
            {
                out.push(p);
            }
        }
    }
    out
}

fn search(
    root: &Path,
    file: &Path,
    find: impl Fn(&str) -> Option<usize>,
) -> Option<(PathBuf, usize)> {
    let start = Instant::now();
    for p in php_files(root, file) {
        if start.elapsed() > BUDGET * 2 {
            return None;
        }
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        if meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        if let Some(line) = find(&text) {
            return Some((p, line));
        }
    }
    None
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

/// Target of the WordPress hook or callback at byte `col` of `line`, in `file`.
pub fn resolve(root: &Path, file: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let col = clamp_col(line, col);
    if !file.extension().is_some_and(|e| e == "php") {
        return None;
    }
    match classify(line, col)? {
        Ref::Callback(name) => search(root, file, |t| find_function(t, &name)),
        Ref::HookRegistered(h) => search(root, file, |t| find_hook_call(t, FIRE, &h)),
        Ref::HookFired(h) => search(root, file, |t| find_hook_call(t, REGISTER, &h)),
        Ref::Shortcode(t) => {
            search(root, file, |text| find_hook_call(text, &["add_shortcode"], &t))
        }
        Ref::Registered(calls, name) => search(root, file, |t| find_hook_call(t, calls, &name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    fn cb(line: &str, needle: &str) -> Option<Ref> {
        classify(line, at(line, needle))
    }

    #[test]
    fn registered_names_classify_from_their_uses() {
        let r = |l: &str, n: &str| cb(l, n);
        assert_eq!(r("$q = new WP_Query(['post_type' => 'book']);", "book"), Some(Ref::Registered(&["register_post_type"], "book".into())));
        assert_eq!(r("if (is_singular('book')) {", "book"), Some(Ref::Registered(&["register_post_type"], "book".into())));
        assert_eq!(r("'taxonomy' => \"genre\",", "genre"), Some(Ref::Registered(&["register_taxonomy"], "genre".into())));
        assert_eq!(r("wp_enqueue_script('my-app', get_theme_file_uri('a.js'));", "my-app"), Some(Ref::Registered(&["wp_register_script"], "my-app".into())));
        assert_eq!(r("wp_enqueue_style( 'my.css' );", "my.css"), Some(Ref::Registered(&["wp_register_style"], "my.css".into())));
        assert_eq!(r("$v = get_option('my_plugin_opt', 1);", "my_plugin_opt"), Some(Ref::Registered(&["add_option", "update_option"], "my_plugin_opt".into())));
        // The key itself, other keys and non-names are not registrations.
        assert_eq!(r("['post_type' => 'book']", "post_type"), None);
        assert_eq!(r("['post_status' => 'publish']", "publish"), None);
        assert_eq!(r("get_option('a b')", "a b"), None);
        assert_eq!(r("foo('book')", "book"), None);
    }

    #[test]
    fn registered_names_find_their_registration() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let plugin = r.join("plugin.php");
        std::fs::write(&plugin, "<?php\nadd_action('init', function () {\n    register_post_type('book', []);\n    register_taxonomy( \"genre\", 'book');\n});\nwp_register_script('my-app', 'a.js');\nadd_option('my_plugin_opt', 1);\n").unwrap();
        let tpl = r.join("single.php");
        let go = |l: &str, n: &str| resolve(r, &tpl, l, at(l, n));
        assert_eq!(go("is_singular('book')", "book"), Some((plugin.clone(), 2)));
        assert_eq!(go("'taxonomy' => 'genre'", "genre"), Some((plugin.clone(), 3)));
        assert_eq!(go("wp_enqueue_script('my-app')", "my-app"), Some((plugin.clone(), 5)));
        assert_eq!(go("get_option('my_plugin_opt')", "my_plugin_opt"), Some((plugin, 6)));
        assert_eq!(go("is_singular('movie')", "movie"), None);
    }

    #[test]
    fn registered_name_lines_with_odd_text_do_not_panic() {
        for l in ["'post_type' =>", "'é' => 'é'", "is_singular('日本", "get_option(\"", "=> '", "'taxonomy'=>'😀'"] {
            for c in 0..=l.len() + 2 {
                let _ = classify(l, c);
            }
        }
    }

    #[test]
    fn shortcode_usage_goes_to_add_shortcode() {
        let l = "echo do_shortcode('[gallery id=\"1\"] and [/box]');";
        assert_eq!(cb(l, "gallery"), Some(Ref::Shortcode("gallery".into())));
        assert_eq!(cb(l, "box"), Some(Ref::Shortcode("box".into())));
        // On an attribute, not the tag.
        assert_eq!(cb(l, "id="), None);
        assert_eq!(cb("echo do_shortcode( \"[my-code]\" );", "my-code"), Some(Ref::Shortcode("my-code".into())));
        assert_eq!(cb("if (shortcode_exists('gallery')) {", "gallery"), Some(Ref::Shortcode("gallery".into())));
        assert_eq!(cb("has_shortcode($post->post_content, 'gallery')", "gallery"), Some(Ref::Shortcode("gallery".into())));
        assert_eq!(cb("echo esc_html('[gallery]');", "gallery"), None);
        assert_eq!(cb("do_shortcode('[ gallery]');", "gallery"), None);
        assert_eq!(cb("shortcode_exists('a b');", "a b"), None);
    }

    #[test]
    fn shortcode_lookup_finds_add_shortcode() {
        let t = "<?php\nfunction x() {}\nadd_shortcode( 'gallery', 'render_gallery' );\nadd_shortcode('galleryx', 'y');\n";
        assert_eq!(find_hook_call(t, &["add_shortcode"], "gallery"), Some(2));
        assert_eq!(find_hook_call(t, &["add_shortcode"], "gal"), None);
    }

    #[test]
    fn shortcode_classify_never_panics() {
        let l = "do_shortcode('[日本語 [é]] [/');";
        for c in 0..=l.len() + 3 {
            let _ = classify(l, c);
        }
    }

    #[test]
    fn classifies_callbacks_and_hook_names() {
        assert_eq!(cb("add_action('init', 'my_init');", "my_init"), Some(Ref::Callback("my_init".into())));
        assert_eq!(cb("add_action('init', 'my_init');", "init'"), Some(Ref::HookRegistered("init".into())));
        assert_eq!(cb("add_filter( \"the_content\", [ $this, 'filter' ] );", "filter'"), Some(Ref::Callback("filter".into())));
        assert_eq!(cb("add_action('x', array(__CLASS__, 'boot'));", "boot"), Some(Ref::Callback("boot".into())));
        assert_eq!(cb("add_action('x', 'Acme\\\\Cls::run');", "run"), Some(Ref::Callback("run".into())));
        assert_eq!(cb("add_shortcode('gallery', 'render_gallery');", "render_gallery"), Some(Ref::Callback("render_gallery".into())));
        assert_eq!(cb("register_activation_hook(__FILE__, 'on_activate');", "on_activate"), Some(Ref::Callback("on_activate".into())));
        assert_eq!(cb("do_action('my_hook', $a);", "my_hook"), Some(Ref::HookFired("my_hook".into())));
        assert_eq!(cb("$v = apply_filters('my_filter', $v);", "my_filter"), Some(Ref::HookFired("my_filter".into())));
        // Not a callback position: a third argument, an unrelated call, a method call.
        assert_eq!(cb("add_action('x', 'cb', 10, 2);", "10"), None);
        assert_eq!(cb("echo 'cb';", "cb"), None);
        assert_eq!(cb("$obj->add_action('x', 'cb');", "'cb'"), None);
        assert_eq!(cb("add_action('x', [$a, $b, 'cb']);", "cb"), None);
    }

    #[test]
    fn finds_functions_and_hook_calls_in_text() {
        let t = "<?php\nclass A {\n  public function run($x) {}\n  function &ref() {}\n}\nfunction my_init() {}\nfunction my_init_two() {}\n";
        assert_eq!(find_function(t, "run"), Some(2));
        assert_eq!(find_function(t, "ref"), Some(3));
        assert_eq!(find_function(t, "my_init"), Some(5));
        assert_eq!(find_function(t, "nope"), None);
        let t = "<?php\nadd_action( 'save_post', 'x' );\ndo_action(\"save_post\", $id);\n";
        assert_eq!(find_hook_call(t, FIRE, "save_post"), Some(2));
        assert_eq!(find_hook_call(t, REGISTER, "save_post"), Some(1));
        assert_eq!(find_hook_call(t, REGISTER, "save"), None);
    }

    #[test]
    fn resolves_across_files_and_skips_vendor() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let main = r.join("plugin.php");
        let inc = r.join("inc/hooks.php");
        std::fs::create_dir_all(inc.parent().unwrap()).unwrap();
        std::fs::create_dir_all(r.join("vendor/x")).unwrap();
        std::fs::write(r.join("vendor/x/v.php"), "<?php function my_cb() {}").unwrap();
        std::fs::write(&main, "<?php\nadd_action('init', 'my_cb');\nadd_action('my_event', 'other');\n").unwrap();
        std::fs::write(&inc, "<?php\n\nfunction my_cb() {}\ndo_action('my_event');\n").unwrap();
        let l = "add_action('init', 'my_cb');";
        assert_eq!(resolve(r, &main, l, at(l, "my_cb")), Some((inc.clone(), 2)));
        let l = "add_action('my_event', 'other');";
        assert_eq!(resolve(r, &main, l, at(l, "my_event")), Some((inc.clone(), 3)));
        let l = "do_action('my_event');";
        assert_eq!(resolve(r, &inc, l, at(l, "my_event")), Some((main.clone(), 2)));
        // Undefined callback, and non-PHP files.
        let l = "add_action('init', 'other');";
        assert_eq!(resolve(r, &main, l, at(l, "other")), None);
        assert_eq!(resolve(r, Path::new("a.js"), l, 20), None);
    }
}
