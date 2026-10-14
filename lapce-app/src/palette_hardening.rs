//! Randomised no-panic and time-bound tests for the pure text scanners behind
//! colour swatches, Blade prop go-to and the component palette. A fixed-seed
//! generator builds lines out of the tokens each scanner cares about plus
//! multibyte filler, so the run is reproducible.
use std::{path::Path, time::Instant};

use crate::{blade_props, color_swatch, component_index};

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
