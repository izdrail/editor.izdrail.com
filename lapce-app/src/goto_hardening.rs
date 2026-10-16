//! Robustness tests shared by the text-based Go to Definition helpers
//! (template, env, WordPress hook, config/lang, route name):
//! every byte column of multibyte and pathological lines, huge lines, files that
//! are not UTF-8, CRLF and BOM files, oversized files and symlink loops. None of
//! them may panic or take noticeable time.
use std::{path::Path, time::Instant};

use crate::{class_refs, composer_pkg, env_refs, laravel_keys, route_names, template_refs, wp_hooks};

fn lines() -> Vec<String> {
    let mut v: Vec<String> = [
        "", "'", "\"", "(", "@include('", "view('é.ü')", "<x-é-ü />", "<x-", "<twig:",
        "<livewire:é", "{% extends 'é' %}", "env('É_X')", "%env(", "${é",
        "add_action('é', 'ñ');", "add_action('x', [", "do_action(\"", "config('é.ü')",
        "__('日本語')", "route('日本')", "😀😀 view('😀') 😀", "\u{0301}\u{0301}'",
        "}}'\"((", "@includeWhen(", "add_action( '", "App\\Élan\\Ünï", "App\\\\\\\\",
        "register_activation_hook(__FILE__, '", "class: 'App\\Ünï\\", "@App\\É::é",
        "Inertia::render('é", "inertia(\"日本/", "Route::inertia('/é', '", "get_template_part('é', '日",
        "get_header(", "require __DIR__ . '/é", "plugin_dir_path( __FILE__ ) . '", "do_shortcode('[é [/",
        "shortcode_exists('é", "asset('é?#", "@vite(['é', '", "mix('/", "wire:click=\"é", "$wire.é",
        "wire:model.live=\"", "x-on:click=\"$wire.", "App\\Controller\\Home::", "::", "\\\\\\", "resource: '../src/'", "\"laravel/é\": \"^1\"", "\"name\": \"a/b",
        "<é-card", "<Ünï.Panel />", "<日本 :x=\"1\">", "<<é", "import é from '@/é", "import('./日本", "require('../é?#", "from \"~/😀",
        "->middleware(['é', 'a:é", "->withoutMiddleware('日本", "Storage::disk('é", "Log::channel(\"日", "DB::connection('😀",
        "Auth::guard(", "\\App\\Models\\É::scopé(", "Post::é(", "É::日本()", "Route::resource('é',", "route('é.index",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.push("a".repeat(200_000));
    v.push("('".repeat(50_000));
    v.push(format!("view('{}')", "é".repeat(100_000)));
    v.push(format!("add_action('{}', '{}')", "x".repeat(50_000), "y".repeat(50_000)));
    v.push("add_action(".repeat(20_000));
    v.push("@include(".repeat(20_000));
    v.push("App\\X".repeat(20_000));
    v
}

fn all_resolvers(root: &Path, line: &str, col: usize) {
    for name in ["a.blade.php", "a.php", "a.html.twig", "a.yaml", "a.xml", "a.neon"] {
        let f = root.join(name);
        let _ = class_refs::resolve(root, &f, line, col);
        let _ = composer_pkg::resolve(root, &root.join("composer.json"), line, col);
        let _ = composer_pkg::resolve(root, &root.join("composer.lock"), line, col);
        let _ = template_refs::resolve(root, &f, line, col);
        let _ = wp_hooks::resolve(root, &f, line, col);
        let _ = crate::blade_props::resolve(root, &f, line, col);
    }
    let _ = crate::inertia_pages::resolve(root, line, col);
    let _ = crate::controller_actions::resolve(root, line, col);
    let _ = crate::blade_slots::resolve(root, line, col);
    let _ = crate::eloquent_scopes::resolve(root, line, col);
    let _ = crate::vue_components::resolve(root, line, col);
    let _ = crate::js_imports::resolve(root, &root.join("a.vue"), line, col);
    let _ = crate::middleware_alias::resolve(root, line, col);
    let _ = crate::config_channels::resolve(root, line, col);
    let _ = crate::wp_templates::resolve(root, &root.join("a.php"), line, col);
    let _ = crate::asset_refs::resolve(root, &root.join("a.blade.php"), line, col);
    let _ = crate::asset_refs::resolve(root, &root.join("a.twig"), line, col);
    let _ = crate::livewire_handlers::resolve(
        root,
        &root.join("resources/views/livewire/a.blade.php"),
        line,
        col,
    );
    let _ = env_refs::resolve(root, line, col);
    let _ = laravel_keys::resolve(root, line, col);
    let _ = route_names::resolve(root, line, col);
}

#[test]
fn no_panic_on_any_column_of_odd_lines() {
    let d = tempfile::tempdir().unwrap();
    for l in lines() {
        let n = l.len();
        let step = if n > 2000 { n / 150 + 1 } else { 1 };
        let mut cols: Vec<usize> = (0..=n + 2).step_by(step).collect();
        cols.extend([n, n.saturating_sub(1), n + 100, usize::MAX]);
        for c in cols {
            all_resolvers(d.path(), &l, c);
        }
    }
}

#[test]
fn pathological_single_calls_stay_fast() {
    let d = tempfile::tempdir().unwrap();
    let start = Instant::now();
    for l in lines() {
        for c in [0, l.len() / 2, l.len()] {
            all_resolvers(d.path(), &l, c);
        }
    }
    assert!(start.elapsed().as_secs() < 5, "took {:?}", start.elapsed());
}

#[test]
fn env_files_with_bom_crlf_and_non_utf8() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    std::fs::write(r.join(".env"), "\u{feff}APP_NAME=Acme\r\nDB_HOST=h\r\n").unwrap();
    assert_eq!(env_refs::find_definition(r, "APP_NAME"), Some((r.join(".env"), 0)));
    assert_eq!(env_refs::find_definition(r, "DB_HOST"), Some((r.join(".env"), 1)));
    // A non-UTF-8 .env is skipped and the next file is used.
    std::fs::write(r.join(".env"), b"A=\xff\xfe\n").unwrap();
    std::fs::write(r.join(".env.example"), "A=1\n").unwrap();
    assert_eq!(env_refs::find_definition(r, "A"), Some((r.join(".env.example"), 0)));
}

#[test]
fn lang_and_config_files_with_crlf_and_binary_content() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    std::fs::create_dir_all(r.join("config")).unwrap();
    std::fs::create_dir_all(r.join("lang/en")).unwrap();
    std::fs::write(r.join("config/app.php"), "<?php\r\nreturn [\r\n  'name' => 'x',\r\n];\r\n").unwrap();
    std::fs::write(r.join("lang/en/m.php"), "<?php\r\nreturn [\r\n 'hi' => 'x',\r\n];\r\n").unwrap();
    let l = "config('app.name')";
    assert_eq!(laravel_keys::resolve(r, l, 10), Some((r.join("config/app.php"), 2)));
    let l = "__('m.hi')";
    assert_eq!(laravel_keys::resolve(r, l, 5), Some((r.join("lang/en/m.php"), 2)));
    std::fs::write(r.join("config/bin.php"), b"\xff\xfe\x00\x01").unwrap();
    assert_eq!(laravel_keys::resolve(r, "config('bin.x')", 10), None);
}

#[test]
fn oversized_files_are_skipped() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    std::fs::create_dir_all(r.join("routes")).unwrap();
    let mut big = String::from("<?php\nRoute::get('/a', fn() => 1)->name('big.one');\n");
    big.push_str(&"// pad\n".repeat(200_000));
    std::fs::write(r.join("routes/web.php"), big).unwrap();
    assert_eq!(route_names::resolve(r, "route('big.one')", 9), None);
    std::fs::write(r.join(".env"), format!("{}A=1\n", "# pad\n".repeat(300_000))).unwrap();
    assert_eq!(env_refs::find_definition(r, "A"), None);
}

#[cfg(unix)]
#[test]
fn symlink_loops_are_not_followed() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    std::fs::create_dir_all(r.join("src/Controller")).unwrap();
    std::os::unix::fs::symlink(r, r.join("src/Controller/loop")).unwrap();
    std::fs::write(r.join("plugin.php"), "<?php\nadd_action('a', 'nope');\n").unwrap();
    let f = r.join("plugin.php");
    let start = Instant::now();
    let l = "add_action('a', 'nope');";
    assert_eq!(wp_hooks::resolve(r, &f, l, 18), None);
    assert_eq!(route_names::resolve(r, "path('missing')", 8), None);
    assert!(start.elapsed().as_secs() < 3);
}

#[test]
fn class_refs_with_bad_composer_and_traversal() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    let f = r.join("services.yaml");
    std::fs::write(r.join("composer.json"), [0xff, 0xfe, b'{']).unwrap();
    assert_eq!(class_refs::resolve(r, &f, "class: App\\Foo", 10), None);
    std::fs::write(
        r.join("composer.json"),
        r#"{"autoload":{"psr-4":{"App\\":"../","":"src/"}}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(r.join("src")).unwrap();
    for l in ["App\\..\\..\\Etc", "App\\\\Foo", "App\\", "\\"] {
        for c in 0..=l.len() {
            let _ = class_refs::resolve(r, &f, l, c);
        }
    }
}

#[test]
fn class_refs_oversized_class_file_is_not_read() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    let f = r.join("services.yaml");
    std::fs::create_dir_all(r.join("src")).unwrap();
    std::fs::write(
        r.join("src/Big.php"),
        "x".repeat(1024 * 1024 + 10),
    )
    .unwrap();
    let t = Instant::now();
    let got = class_refs::resolve(r, &f, "class: App\\Big", 10);
    assert_eq!(got, Some((r.join("src/Big.php"), 0)));
    assert!(t.elapsed().as_secs() < 2);
}

#[test]
fn new_resolvers_skip_oversized_and_hostile_trees() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    // Livewire class over 1 MB is not read.
    std::fs::create_dir_all(r.join("app/Livewire")).unwrap();
    std::fs::create_dir_all(r.join("resources/views/livewire")).unwrap();
    let mut big = String::from("<?php\nclass Big {\n public function save() {}\n}\n");
    big.push_str(&"// pad\n".repeat(200_000));
    std::fs::write(r.join("app/Livewire/Big.php"), big).unwrap();
    let view = r.join("resources/views/livewire/big.blade.php");
    let l = "<b wire:click=\"save\">";
    assert_eq!(crate::livewire_handlers::resolve(r, &view, l, 17), None);
    // A directory named like an asset or page is not a file.
    std::fs::create_dir_all(r.join("public/css/app.css")).unwrap();
    std::fs::create_dir_all(r.join("resources/js/Pages/Users.vue")).unwrap();
    let blade = r.join("v.blade.php");
    assert_eq!(crate::asset_refs::resolve(r, &blade, "asset('css/app.css')", 10), None);
    assert_eq!(crate::inertia_pages::resolve(r, "inertia('Users')", 10), None);
    // Odd file names in a WordPress theme.
    std::fs::write(r.join("style.css"), "/* Theme Name: x */").unwrap();
    let f = r.join("page.php");
    for l in [
        "get_template_part('a\\0b');",
        "get_template_part('con\u{301}tent', '\u{1F600}');",
        "locate_template('');",
    ] {
        for c in 0..l.len() + 2 {
            let _ = crate::wp_templates::resolve(r, &f, l, c);
        }
    }
}
