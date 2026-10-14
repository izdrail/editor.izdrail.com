//! Randomised no-panic and time-bound tests for the pure text scanners behind
//! colour swatches, Blade prop go-to and the component palette. A fixed-seed
//! generator builds lines out of the tokens each scanner cares about plus
//! multibyte filler, so the run is reproducible.
use std::{path::Path, time::Instant};

use crate::{php_imports, test_index, php_organize, tw_sort, blade_props, color_swatch, component_index, env_index, hover_help, model_index};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[(self.next() % v.len() as u64) as usize]
    }
}

fn soup(rng: &mut Rng, tokens: &[&str], n: usize) -> String {
    (0..n).map(|_| rng.pick(tokens)).collect()
}

const TOKENS: &[&str] = &[
    "#", "#fff", "#12", "#ffffffff", "rgb(", "rgba(", "hsl(", ")", ",", " ", "/", "%", "deg", "1", "255", "-3",
    ".5", "é", "日", "😀", "<x-", "<x-card", " type=", "\"", "'", "{{", "}}", ">", "/>", "\n", ":", "::", "-",
    "wire:model", "@props(", "[", "]", "=>", "a", "NaN", "1e999",
];

#[test]
fn color_swatch_random_lines() {
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let start = Instant::now();
    for _ in 0..3000 {
        let n = (rng.next() % 60) as usize;
        let line = soup(&mut rng, TOKENS, n);
        let found = color_swatch::find(&line);
        assert!(found.len() <= 12);
        for s in &found {
            assert!(s.col < line.len().max(1));
            assert!(line.is_char_boundary(s.col));
        }
    }
    assert!(start.elapsed().as_secs() < 5);
}

#[test]
fn color_swatch_huge_lines_are_fast() {
    let start = Instant::now();
    for line in ["rgb(".repeat(500_000), "#fff ".repeat(500_000), "é#fff".repeat(300_000), "hsl(1,".repeat(300_000)] {
        assert!(color_swatch::find(&line).len() <= 12);
    }
    assert!(start.elapsed().as_secs() < 5);
}

#[test]
fn blade_props_random_windows() {
    let mut rng = Rng(0xD1B54A32D192ED03);
    let start = Instant::now();
    for _ in 0..2000 {
        let n = (rng.next() % 80) as usize;
        let text = soup(&mut rng, TOKENS, n);
        for at in 0..text.len() + 2 {
            let _ = blade_props::tag_and_attr(&text, at);
        }
        let _ = blade_props::prop_line(&text, "type");
        let _ = blade_props::ctor_line(&text, "type");
    }
    assert!(start.elapsed().as_secs() < 10);
}

#[test]
fn blade_props_huge_window_is_fast() {
    let start = Instant::now();
    let text = format!("<x-card {}", "a=\"b\" ".repeat(20_000));
    for at in [0, 10, text.len() / 2, text.len()] {
        let _ = blade_props::tag_and_attr(&text, at);
    }
    let _ = blade_props::prop_line(&"@props(".repeat(50_000), "a");
    assert!(start.elapsed().as_secs() < 5);
}

#[test]
fn component_index_survives_odd_trees() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    // Odd names, deep nesting and a file where a directory is expected.
    let deep = r.join("resources/views/components/a/b/c/d/e/f/g/h/i/j");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("deep.blade.php"), "x").unwrap();
    std::fs::write(r.join("resources/views/components/é-日.blade.php"), "x").unwrap();
    std::fs::write(r.join("resources/views/components/.blade.php"), "x").unwrap();
    std::fs::create_dir_all(r.join("app/View")).unwrap();
    std::fs::write(r.join("app/View/Components"), "not a dir").unwrap();
    let got = component_index::discover(r);
    assert!(got.iter().any(|t| t.label.starts_with("<x-é-日>")));
    assert!(!got.iter().any(|t| t.label.contains("deep")), "depth limit");
    // Many files stay under the cap and finish quickly.
    let many = r.join("resources/views/livewire");
    std::fs::create_dir_all(&many).unwrap();
    for i in 0..6000 {
        std::fs::write(many.join(format!("c{i}.blade.php")), "").unwrap();
    }
    let start = Instant::now();
    assert!(component_index::discover(r).len() <= 5000);
    assert!(start.elapsed().as_secs() < 10);
    assert!(component_index::discover(Path::new("/nonexistent/x")).is_empty());
}

const PHP_TOKENS: &[&str] = &[
    "<?php", "return", "[", "]", "'", "\"", "=>", ",", ";", "//", "/*", "*/", "#", "\n", " ", "class ", "A", " extends ",
    "Model", "function ", "public ", "$this->", "hasMany", "belongsTo", "(", ")", "::class", "{", "}", "é", "日",
    "KEY", "=", "export ", "\\",
];

#[test]
fn config_env_and_model_parsers_random_input() {
    let mut rng = Rng(0xA0761D6478BD642F);
    let start = Instant::now();
    for _ in 0..3000 {
        let n = (rng.next() % 120) as usize;
        let t = soup(&mut rng, PHP_TOKENS, n);
        let _ = env_index::parse_config(&t);
        let _ = env_index::parse_env(&t);
        let _ = model_index::parse(&t);
    }
    for t in ["[".repeat(300_000), "'[".repeat(200_000), "/*".repeat(200_000), "return [".to_string() + &"'a' => [".repeat(50_000)] {
        let _ = env_index::parse_config(&t);
    }
    assert!(start.elapsed().as_secs() < 10);
}

#[test]
fn hover_help_random_windows() {
    let mut rng = Rng(0xE7037ED1A0B428DB);
    let f = Path::new("a.blade.php");
    let start = Instant::now();
    for _ in 0..1500 {
        let n = (rng.next() % 60) as usize;
        let t = soup(&mut rng, TOKENS, n) + &soup(&mut rng, &["@", "@if", "@foreach", "@@", "a@b"], 6);
        for at in 0..t.len() + 2 {
            let _ = hover_help::help(None, f, &t, at);
            let _ = hover_help::help(Some(Path::new("/nonexistent")), f, &t, at);
        }
    }
    assert!(start.elapsed().as_secs() < 10);
}

const IMPORT_TOKENS: &[&str] = &[
    "<?php\n", "namespace App;\n", "use ", "App\\Models\\User", "Foo\\Bar", " as ", "Baz", ";", "\n", "\r\n", " ",
    "function ", "{", "}", "class A", "const ", "use function ", "use const ", "\\", "/*", "*/", "//", "$u", "User",
    "é", "日", "#", "'", "\"", "(", ")", "new ", "::", "<", ">",
];

fn apply(text: &str, edits: &[(std::ops::Range<usize>, String)]) -> String {
    let mut out = String::new();
    let mut pos = 0;
    for (r, rep) in edits {
        assert!(r.start >= pos && r.end >= r.start && r.end <= text.len(), "edit order or bounds");
        assert!(text.is_char_boundary(r.start) && text.is_char_boundary(r.end), "char boundary");
        out.push_str(&text[pos..r.start]);
        out.push_str(rep);
        pos = r.end;
    }
    out.push_str(&text[pos..]);
    out
}

#[test]
fn php_organize_random_files_keep_invariants() {
    let mut rng = Rng(0x8EBC6AF09C88C6E3);
    let start = Instant::now();
    for _ in 0..4000 {
        let n = (rng.next() % 80) as usize;
        let t = soup(&mut rng, IMPORT_TOKENS, n);
        if let Some(o) = php_organize::organize(&t) {
            let once = apply(&t, &o.edits);
            // Running it on its own output must change nothing further.
            if let Some(o2) = php_organize::organize(&once) {
                assert_eq!(apply(&once, &o2.edits), once, "not idempotent for {t:?}");
            }
        }
    }
    let big = "use A\\B;\n".repeat(200_000);
    let _ = php_organize::organize(&big);
    let _ = php_organize::organize(&"use ".repeat(300_000));
    assert!(start.elapsed().as_secs() < 15);
}

const TW_TOKENS: &[&str] = &[
    "p-4", "flex", "md:p-2", "hover:bg-red-500", "dark:md:hover:text-sm", "!font-bold", "-mt-2", "w-[calc(100%-1rem)]",
    "[&>*]:p-1", "group-hover:flex", "sm:", ":", "::", "bg-", "é", "日", " ", "  ", "\t", "\n", "{{", "$x", "@if",
    "class=\"", "className='", "\"", "'", "<div ", ">", "class=", "xxx-yyy", "container", "text-[#fff]", "/",
];

#[test]
fn tw_sort_random_values_keep_invariants() {
    let mut rng = Rng(0x589965CC75374CC3);
    let start = Instant::now();
    for _ in 0..4000 {
        let n = (rng.next() % 40) as usize;
        let t = soup(&mut rng, TW_TOKENS, n);
        if let Some(sorted) = tw_sort::sort_classes(&t) {
            let mut a: Vec<_> = t.split_whitespace().collect();
            let mut b: Vec<_> = sorted.split_whitespace().collect();
            a.sort();
            b.sort();
            assert_eq!(a, b, "tokens lost or invented for {t:?}");
            assert!(tw_sort::sort_classes(&sorted).is_none(), "not idempotent for {t:?}");
        }
        let edits = tw_sort::sort_attributes(&t, 0..t.len());
        let _ = apply(&t, &edits);
        let sub = t.len() / 2;
        let mut e = sub;
        while !t.is_char_boundary(e) {
            e -= 1;
        }
        let _ = apply(&t, &tw_sort::sort_attributes(&t, 0..e));
    }
    let big = format!("<div class=\"{}\">", "p-4 flex md:p-2 ".repeat(100_000));
    let _ = tw_sort::sort_attributes(&big, 0..big.len());
    let _ = tw_sort::sort_attributes(&"class=\"".repeat(200_000), 0..1_000_000);
    assert!(start.elapsed().as_secs() < 15);
}

#[test]
fn php_imports_and_test_index_random_files() {
    let mut rng = Rng(0x1D8E4E27C47D124F);
    let start = Instant::now();
    for _ in 0..4000 {
        let n = (rng.next() % 80) as usize;
        let t = soup(&mut rng, IMPORT_TOKENS, n);
        if let Some((off, ins)) = php_imports::insertion(&t, "App\\Models\\User") {
            assert!(off <= t.len() && t.is_char_boundary(off), "insertion offset for {t:?}");
            assert!(ins.contains("use App"));
        }
        for col in 0..t.len().min(60) + 2 {
            let _ = php_imports::name_at(&t, col);
        }
        let _ = test_index::parse(&soup(&mut rng, PHP_TOKENS, n));
        let _ = test_index::parse(&soup(&mut rng, &["#[Test]", "@test", "it('x'", "test(\"y\"", "function ", "test_a", "public ", "\n", "class A"], n));
    }
    assert!(start.elapsed().as_secs() < 15);
}
