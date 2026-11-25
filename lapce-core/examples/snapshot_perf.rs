//! On-demand large-file snapshot performance measurement (item 447).
//!
//! Legacy Syntax::parse mode is manual. CI runs a small --compare smoke. Use
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
        (Some(b), Some(a)) => {
            format!("{:+.1} MiB", (a as f64 - b as f64) / MIB as f64)
        }
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
        Case {
            name: "pure php 1 MiB",
            language: LapceLanguage::Php,
            text: pure_php(MIB),
            iters: 5,
        },
        Case {
            name: "pure php 5 MiB",
            language: LapceLanguage::Php,
            text: pure_php(5 * MIB),
            iters: 3,
        },
        Case {
            name: "mixed    1 MiB",
            language: LapceLanguage::Php,
            text: mixed_php_html(MIB),
            iters: 5,
        },
        Case {
            name: "mixed    5 MiB",
            language: LapceLanguage::Php,
            text: mixed_php_html(5 * MIB),
            iters: 3,
        },
        Case {
            name: "html     1 MiB",
            language: LapceLanguage::Html,
            text: html_doc(MIB),
            iters: 5,
        },
        Case {
            name: "html     5 MiB",
            language: LapceLanguage::Html,
            text: html_doc(5 * MIB),
            iters: 3,
        },
    ]
}

fn main() {
    if std::env::args().any(|arg| arg == "--compare") {
        #[cfg(feature = "logos-php")]
        {
            let quick = std::env::args().any(|arg| arg == "--quick");
            let args = std::env::args().collect::<Vec<_>>();
            let number = |flag: &str| {
                args.iter()
                    .position(|arg| arg == flag)
                    .and_then(|index| args.get(index + 1))
                    .and_then(|value| value.parse::<usize>().ok())
            };
            compare(
                number("--mib")
                    .map(|n| n.clamp(1, 16) * MIB)
                    .unwrap_or(if quick { 64 * 1024 } else { MIB }),
                number("--iters")
                    .unwrap_or(if quick { 3 } else { 7 })
                    .clamp(1, 25),
            );
            return;
        }
        #[cfg(not(feature = "logos-php"))]
        {
            eprintln!("--compare requires --features logos-php");
            std::process::exit(2);
        }
    }

    println!(
        "snapshot_perf: full Syntax::parse cost (lexilla + fold levels + ranges)"
    );
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

/// Raw lexer comparison. Native snapshots include their native fold pass;
/// Logos cache timings do not include the separately debounced fold refresh.
#[cfg(feature = "logos-php")]
fn compare(target: usize, iters: usize) {
    use gravity_lex::{
        cooperative::{Session, Step},
        incremental::{Cache, Update},
    };
    use lexilla_rs::Snapshot;
    use std::sync::{atomic::AtomicUsize, Arc};
    fn median(mut values: Vec<f64>) -> f64 {
        values.sort_by(f64::total_cmp);
        values[values.len() / 2]
    }
    fn elapsed(start: Instant) -> f64 {
        start.elapsed().as_secs_f64() * 1000.0
    }
    let inputs = [
        ("php", false, pure_php(target)),
        ("mixed", false, mixed_php_html(target)),
        ("blade", true, "<x-card>{{ $title }}</x-card>\n@if($x) yes @endif\n".repeat(target / 46 + 1)),
        ("laravel", false, format!("<?php\n{}", "Route::get('/users', fn () => User::query()->get());\n".repeat(target / 49 + 1))),
        ("symfony", false, format!("<?php\n{}", "#[Route('/users')] function index(Request $request): Response { return new Response('ok'); }\n".repeat(target / 88 + 1))),
    ];
    let mut results = Vec::new();
    for (name, blade, text) in inputs {
        if std::env::var("SNAPSHOT_PERF_CASE").is_ok_and(|value| value != name) {
            continue;
        }
        if let Ok(backend) = std::env::var("SNAPSHOT_PERF_SYNTAX") {
            assert!(backend == "logos" || backend == "lexilla");
            // Choose the backend before launch, never mutate process environment.
            assert_eq!(
                std::env::var("GRAVITY_LEXER").as_deref(),
                Ok(backend.as_str()),
                "launch with GRAVITY_LEXER matching SNAPSHOT_PERF_SYNTAX"
            );
            let make = || {
                let mut syntax = Syntax::from_language(if blade {
                    LapceLanguage::Blade
                } else {
                    LapceLanguage::Php
                });
                syntax.project.laravel = name == "laravel" || blade;
                syntax.project.symfony_app = name == "symfony";
                syntax
            };
            let mut initial = Vec::new();
            let mut folds = Vec::new();
            let mut tail = Vec::new();
            for _ in 0..iters {
                let mut syntax = make();
                let start = Instant::now();
                syntax.parse(1, Rope::from(text.as_str()), None);
                initial.push(elapsed(start));
                assert_eq!(syntax.text.len(), text.len());
                assert_eq!(
                    syntax.lexilla.as_ref().unwrap().lexer().ends_with("logos"),
                    backend == "logos"
                );
                let start = Instant::now();
                syntax.refresh_native_folding();
                folds.push(elapsed(start));
                let mut changed = text.clone();
                changed.push(' ');
                let start = Instant::now();
                syntax.parse(2, Rope::from(changed.as_str()), None);
                tail.push(elapsed(start));
                assert_eq!(syntax.rev, 2);
            }
            results.push(serde_json::json!({"case":name,"backend":backend,"bytes":text.len(),"iterations":iters,"syntax_initial_median_ms":median(initial),"syntax_tail_median_ms":median(tail),"separate_fold_refresh_median_ms":median(folds)}));
            continue;
        }
        let native_name = if blade { "blade" } else { "php" };
        // Raw PHP/Blade vocabulary only, same for every framework case.
        let words = lapce_core::lexilla::PHP_WORD_LISTS;
        if let Ok(backend) = std::env::var("SNAPSHOT_PERF_RSS_BACKEND") {
            assert!(backend == "logos" || backend == "lexilla");
            let before = rss_bytes();
            let after = if backend == "logos" {
                let retained = Cache::new(text.clone(), blade);
                let rss = rss_bytes();
                std::hint::black_box(&retained);
                rss
            } else {
                let retained = Snapshot::lex(native_name, &text, words).unwrap();
                let rss = rss_bytes();
                std::hint::black_box(&retained);
                rss
            };
            results.push(serde_json::json!({"case":name,"backend":backend,"bytes":text.len(),"rss_before":before,"rss_retained":after,"rss_delta_bytes":before.zip(after).map(|(b,a)|a as i64-b as i64)}));
            continue;
        }
        let full = if blade {
            gravity_lex::lex_blade(&text)
        } else {
            gravity_lex::lex(&text)
        };
        let mut native_times = Vec::new();
        let mut logos_times = Vec::new();
        for iteration in 0..iters {
            // Alternate order to reduce systematic warm-cache bias.
            let mut native = || {
                let start = Instant::now();
                std::hint::black_box(
                    Snapshot::lex(native_name, &text, words).unwrap(),
                );
                native_times.push(elapsed(start));
            };
            let mut logos = || {
                let start = Instant::now();
                std::hint::black_box(Cache::new(text.clone(), blade));
                logos_times.push(elapsed(start));
            };
            if iteration % 2 == 0 {
                native();
                logos();
            } else {
                logos();
                native();
            }
        }
        let mut edits = Vec::new();
        for (position, offset) in [
            ("front", 0),
            ("middle", text.len() / 2),
            ("tail", text.len()),
        ] {
            let mut offset = offset;
            while !text.is_char_boundary(offset) {
                offset -= 1;
            }
            let mut changed = text.clone();
            changed.insert_str(offset, " ");
            let reference = if blade {
                gravity_lex::lex_blade(&changed)
            } else {
                gravity_lex::lex(&changed)
            };
            let mut native_times = Vec::new();
            let mut logos_times = Vec::new();
            let mut relexed = 0;
            for _ in 0..iters {
                let mut cache = Cache::new(text.clone(), blade);
                let start = Instant::now();
                std::hint::black_box(
                    Snapshot::lex(native_name, &changed, words).unwrap(),
                );
                native_times.push(elapsed(start));
                // Unlimited allowance characterizes the cache algorithm, not
                // the production 64 KiB defer + cooperative fallback policy.
                let start = Instant::now();
                let update = cache.update(changed.clone(), usize::MAX);
                let time = elapsed(start);
                match update {
                    Update::Ready {
                        output,
                        relexed_bytes,
                        ..
                    } => {
                        logos_times.push(time);
                        relexed = relexed_bytes;
                        assert_eq!(output.styles, reference.styles);
                    }
                    _ => panic!("unexpected defer"),
                }
            }
            edits.push(serde_json::json!({"position":position,"native_full_snapshot_median_ms":median(native_times),"logos_cache_update_median_ms":median(logos_times),"logos_relexed_bytes":relexed}));
        }
        let mut session =
            Session::new(text.clone(), blade, Arc::new(AtomicUsize::new(0)));
        let mut max_step = 0f64;
        let mut yields = 0;
        let cooperative_start = Instant::now();
        loop {
            let start = Instant::now();
            let step = session.step(16 * 1024);
            max_step = max_step.max(elapsed(start));
            match step {
                Step::Yielded { scanned } => {
                    assert!(scanned <= 16 * 1024);
                    yields += 1;
                }
                Step::Complete(output) => {
                    assert_eq!(output.styles, full.styles);
                    break;
                }
                Step::Cancelled => panic!("cancelled"),
            }
        }
        let cooperative_total = elapsed(cooperative_start);
        let cancel = Arc::new(AtomicUsize::new(0));
        let mut cancelled = Session::new(text.clone(), blade, cancel.clone());
        assert!(matches!(cancelled.step(1024), Step::Yielded { .. }));
        cancel.store(1, std::sync::atomic::Ordering::Release);
        let start = Instant::now();
        assert!(matches!(cancelled.step(1024), Step::Cancelled));
        let cancel_ms = elapsed(start);
        let native = Snapshot::lex(native_name, &text, words).unwrap();
        let imported = if blade {
            Snapshot::from_blade_styles(&text, full.styles.clone())
        } else {
            Snapshot::from_mixed_php_styles(&text, full.styles.clone())
        }
        .unwrap();
        let (mut comparable, mut same) = (0, 0);
        for (at, &style) in native.styles().iter().enumerate() {
            let a = native.scope(style);
            let b = imported.scope(full.styles[at]);
            if a.is_some() || b.is_some() {
                comparable += 1;
                if a == b {
                    same += 1;
                }
            }
        }
        results.push(serde_json::json!({"case":name,"bytes":text.len(),"iterations":iters,"native_initial_median_ms":median(native_times),"logos_initial_median_ms":median(logos_times),"edits":edits,"worker_step_max_observed_ms":max_step,"worker_yields":yields,"worker_total_ms":cooperative_total,"worker_cancel_observed_ms":cancel_ms,"native_scope_agreement_diagnostic":if comparable==0{1.0}else{same as f64/comparable as f64}}));
    }
    assert!(!results.is_empty(), "unknown SNAPSHOT_PERF_CASE");
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"schema":2,"mode":if std::env::var_os("SNAPSHOT_PERF_SYNTAX").is_some(){"syntax-worker"}else if std::env::var_os("SNAPSHOT_PERF_RSS_BACKEND").is_some(){"isolated-rss"}else{"raw-lexer"},"profile":if cfg!(debug_assertions){"debug"}else{"release"},"measurement":"raw lexers; native snapshots include folding, Logos cache excludes deferred native folding and framework overlays","memory":"RSS mode requires a fresh process per backend and case; delta is retained RSS, not allocation or peak","accuracy":"scope agreement is diagnostic only; token_get_all corpus is the independent accuracy gate","responsiveness":"worker step and cancellation proxies, not UI frame or input latency; no timing threshold","default_decision":"KEEP_LEXILLA","reason":"no end-to-end UI benefit established","results":results})).unwrap());
}
