//! cargo run --release -p lapce-index --example php_project_perf -- 3000
//! Fixture/store are kept in a printed temp directory for inspection.
use lapce_index::php::cache::Engine;
use std::{collections::BTreeSet, sync::atomic::AtomicBool, time::Instant};
fn main() {
    let count = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(3000);
    let root =
        std::env::temp_dir().join(format!("php-index-perf-{}", std::process::id()));
    let project = root.join("project");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join("composer.json"),
        r#"{"autoload":{"psr-4":{"App\\":"src"}}}"#,
    )
    .unwrap();
    for i in 0..count {
        std::fs::write(project.join(format!("src/C{i}.php")),format!("<?php namespace App; #[Entity] class C{i} {{ public const ID={i}; private string $name; public function save(): void {{}} }}")).unwrap();
    }
    let cancel = AtomicBool::new(false);
    let mut engine = Engine::open(&root.join("index")).unwrap();
    let start = Instant::now();
    let initial = engine
        .reconcile(&project, &BTreeSet::new(), &cancel)
        .unwrap();
    let initial_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    let warm = engine
        .reconcile(&project, &BTreeSet::new(), &cancel)
        .unwrap();
    let warm_ms = start.elapsed().as_secs_f64() * 1000.;
    let path = project.join("src/C0.php");
    std::fs::write(&path, "<?php namespace App; class Changed {}").unwrap();
    let start = Instant::now();
    let edit = engine
        .reconcile(&project, &BTreeSet::from([path]), &cancel)
        .unwrap();
    let edit_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(engine.symbols("C19", 100));
    }
    let query_ms = start.elapsed().as_secs_f64() * 1000.;
    println!(
        "fixture={} files={count} initial_ms={initial_ms:.3} warm_ms={warm_ms:.3} edit_ms={edit_ms:.3} thousand_prefix_ms={query_ms:.3} parsed={}/{}/{} reused={}/{}",
        root.display(),
        initial.parsed,
        warm.parsed,
        edit.parsed,
        warm.reused,
        edit.reused
    );
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status
            .lines()
            .filter(|s| s.starts_with("VmHWM:") || s.starts_with("VmRSS:"))
        {
            println!("{line}");
        }
    }
}
