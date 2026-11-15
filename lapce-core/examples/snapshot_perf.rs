//! On-demand large-file snapshot performance measurement (item 447).
//!
//! Not part of the test suite and not run by CI: invoke explicitly with
//! `cargo run --release -p lapce-core --example snapshot_perf`.
//!
//! Measures the work `Syntax::parse` performs when the editor opens a
//! file: the Lexilla highlight snapshot (including the Fold pass that
//! produces fold levels since item 446), the Textmate fallback decision
//! and folding-range derivation. Corpora are generated deterministically
//! so runs are comparable across machines and revisions.

use std::time::{Duration, Instant};

use lapce_core::language::LapceLanguage;
use lapce_core::syntax::Syntax;
use lapce_xi_rope::Rope;

const MIB: usize = 1024 * 1024;

fn rss_bytes() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb: usize = status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    Some(kb * 1024)
}

/// Pure PHP: classes, functions, interpolation, heredoc, comments.
fn pure_php(target: usize) -> String {
    let mut text = String::from("<?php\n");
    let mut i = 0usize;
    while text.len() < target {
        text.push_str(&format!(
            "namespace App\\Module{i};\n\
             \n\
             /** Render block {i}. */\n\
             class Renderer{i}\n\
             {{\n\
             \x20   private string $prefix = 'item-{i}';\n\
             \n\
             \x20   public function render(array $items): string\n\
             \x20   {{\n\
             \x20       $out = \"<ul class=\\\"list-{i}\\\">\";\n\
             \x20       foreach ($items as $idx => $item) {{\n\
             \x20           if ($idx % 2 === 0) {{\n\
             \x20               $out .= \"<li>{{$item}}</li>\";\n\
             \x20           }} else {{\n\
             \x20               $out .= '<li class=\"odd\">' . htmlspecialchars($item) . '</li>';\n\
             \x20           }}\n\
             \x20       }}\n\
             \x20       // trailing note for block {i}\n\
             \x20       $out .= \"</ul>\";\n\
             \x20       return <<<HTML\n\
             \x20       <div class=\"wrapper-{i}\">{{$out}}</div>\n\
             \x20       HTML;\n\
             \x20   }}\n\
             }}\n\
             \n"
        ));
        i += 1;
    }
    text
}

/// Mixed PHP/HTML: markup with interleaved `<?php ?>` islands, the
/// common WordPress/theme template shape.
fn mixed_php_html(target: usize) -> String {
    let mut text = String::from("<!doctype html>\n<html>\n<body>\n");
    let mut i = 0usize;
    while text.len() < target {
        text.push_str(&format!(
            "<section class=\"row-{i}\">\n\
             \x20 <h2>Section {i}</h2>\n\
             \x20 <?php if ($items->{i}): ?>\n\
             \x20 <ul>\n\
             \x20 <?php foreach ($items->{i} as $entry): ?>\n\
             \x20   <li><?= esc_html($entry->title) ?></li>\n\
             \x20 <?php endforeach; ?>\n\
             \x20 </ul>\n\
             \x20 <?php else: ?>\n\
             \x20 <p>No entries for section {i}.</p>\n\
             \x20 <?php endif; ?>\n\
             </section>\n"
        ));
        i += 1;
    }
    text.push_str("</body>\n</html>\n");
    text
}

/// Plain HTML: nested structural tags for the tag-folder.
fn html_doc(target: usize) -> String {
    let mut text = String::from("<!doctype html>\n<html>\n<body>\n");
    let mut i = 0usize;
    while text.len() < target {
        text.push_str(&format!(
            "<div class=\"card-{i}\">\n\
             \x20 <article>\n\
             \x20   <header><h3>Card {i}</h3></header>\n\
             \x20   <p>Body copy for card number {i} with <a href=\"/{i}\">a link</a>.</p>\n\
             \x20   <footer><span>Footer {i}</span></footer>\n\
             \x20 </article>\n\
             </div>\n"
        ));
        i += 1;
    }
    text.push_str("</body>\n</html>\n");
    text
}

fn measure(name: &str, language: LapceLanguage, text: String, iters: usize) {
    let bytes = text.len();
    let lines = text.lines().count();
    let rss_before = rss_bytes();

    // First snapshot stays alive so the RSS delta includes one retained
    // snapshot and its rope; later iterations measure steady-state time.
    let mut times: Vec<Duration> = Vec::with_capacity(iters);
    let mut retained: Option<Syntax> = None;
    let mut fold_ranges = 0usize;
    for iter in 0..iters {
        let mut syntax = Syntax::from_language(language);
        let start = Instant::now();
        syntax.parse(1, Rope::from(text.as_str()), None);
        times.push(start.elapsed());
        fold_ranges = syntax.folding_ranges.len();
        if iter == 0 {
            retained = Some(syntax);
        }
    }

    let rss_after = rss_bytes();
    times.sort();
    let min = times[0];
    let median = times[times.len() / 2];
    let mib = bytes as f64 / MIB as f64;
    let rss_delta = match (rss_before, rss_after) {
        (Some(b), Some(a)) => format!("{:+.1} MiB", (a as f64 - b as f64) / MIB as f64),
        _ => "n/a".to_string(),
    };
    println!(
        "{name}: {mib:.1} MiB, {lines} lines, {iters} iters | \
         min {:.0} ms, median {:.0} ms ({:.0} ms/MiB) | \
         {fold_ranges} fold ranges | retained RSS delta {rss_delta}",
        min.as_secs_f64() * 1000.0,
        median.as_secs_f64() * 1000.0,
        median.as_secs_f64() * 1000.0 / mib,
    );
    drop(retained);
}

struct Case {
    name: &'static str,
    language: LapceLanguage,
    text: String,
    iters: usize,
}

fn cases() -> Vec<Case> {
    vec![
        Case { name: "pure php 1 MiB", language: LapceLanguage::Php, text: pure_php(MIB), iters: 5 },
        Case { name: "pure php 5 MiB", language: LapceLanguage::Php, text: pure_php(5 * MIB), iters: 3 },
        Case { name: "mixed    1 MiB", language: LapceLanguage::Php, text: mixed_php_html(MIB), iters: 5 },
        Case { name: "mixed    5 MiB", language: LapceLanguage::Php, text: mixed_php_html(5 * MIB), iters: 3 },
        Case { name: "html     1 MiB", language: LapceLanguage::Html, text: html_doc(MIB), iters: 5 },
        Case { name: "html     5 MiB", language: LapceLanguage::Html, text: html_doc(5 * MIB), iters: 3 },
    ]
}

fn main() {
    println!("snapshot_perf: full Syntax::parse cost (lexilla + fold levels + ranges)");
    let cases = cases();
    // `--only N` runs a single case: fresh processes give clean RSS deltas.
    if let Some(idx) = std::env::args()
        .nth(2)
        .filter(|_| std::env::args().nth(1).as_deref() == Some("--only"))
        .and_then(|n| n.parse::<usize>().ok())
    {
        let case = &cases[idx];
        measure(case.name, case.language, case.text.clone(), case.iters);
        return;
    }
    for case in &cases {
        measure(case.name, case.language, case.text.clone(), case.iters);
    }
}
