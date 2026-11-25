//! Reproducible min/median component measurements, never UI latency.
use lapce_php::{Document, byte_to_point};
use lapce_xi_rope::Rope;
use std::{sync::atomic::AtomicUsize, time::Instant};
fn stats(mut values: Vec<f64>) -> (f64, f64) {
    values.sort_by(f64::total_cmp);
    (values[0], values[values.len() / 2])
}
fn main() {
    let source = format!(
        "<?php\n{}",
        (0..40000)
            .map(|n| format!("$value{n}='SELECT id FROM users';\n"))
            .collect::<String>()
    );
    let cancel = AtomicUsize::new(0);
    let mut initial = Vec::new();
    let mut lookups = Vec::new();
    let mut edits = Vec::new();
    let mut regions = 0;
    let mut rescanned = 0;
    for _ in 0..5 {
        let start = Instant::now();
        let doc =
            Document::new(Rope::from(source.as_str()), false, &cancel).unwrap();
        initial.push(start.elapsed().as_secs_f64() * 1000.);
        regions = doc.index().regions().len();
        let start = Instant::now();
        for index in 0..1_000_000 {
            std::hint::black_box(doc.index().at(index % source.len()));
        }
        lookups.push(start.elapsed().as_secs_f64() * 1000.);
        let at = source.find("value20000").unwrap();
        let mut changed = source.clone();
        changed.insert(at, 'x');
        let old = Rope::from(source.as_str());
        let new = Rope::from(changed.as_str());
        let edit = tree_sitter::InputEdit {
            start_byte: at,
            old_end_byte: at,
            new_end_byte: at + 1,
            start_position: byte_to_point(&old, at).unwrap(),
            old_end_position: byte_to_point(&old, at).unwrap(),
            new_end_position: byte_to_point(&new, at + 1).unwrap(),
        };
        let start = Instant::now();
        let updated = doc.update(new, &[edit], &cancel).unwrap();
        edits.push(start.elapsed().as_secs_f64() * 1000.);
        rescanned = updated.rescanned_units;
    }
    println!(
        "profile={} iterations=5 bytes={} regions={} initial_min_median_ms={:?} million_lookup_min_median_ms={:?} edit_min_median_ms={:?} rescanned_units={}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        source.len(),
        regions,
        stats(initial),
        stats(lookups),
        stats(edits),
        rescanned
    );
}
