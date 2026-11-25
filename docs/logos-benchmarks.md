# Logos vs Lexilla benchmark gate (#15)

Benchmark recommendation at measurement time: **keep Lexilla as default**.
These numbers did not justify a performance-based switch. A later explicit
product decision selected Logos as default with a persisted Settings choice;
see [the current guide](logos-lexer.md). Measurements and raw decisions below
remain unchanged historical evidence, not a claim that Logos became faster.

## Reproduce

Run `bash scripts/benchmark-logos.sh [output-directory]` from the repository.
It builds the release example with `logos-php` and records deterministic 1 and
5 MiB-class PHP, mixed HTML/PHP, Blade, Laravel-shaped and Symfony-shaped inputs.
No fixture PHP is executed. Three iterations per timing, median reported.
`--compare --quick` gives a 64 KiB debug smoke; CI checks shape, cancellation,
cooperative budget bounds and incremental/full byte-style equality, not timings.
The legacy `--only N` Syntax::parse benchmark still works without the feature.

Recorded run: 2026-11-25, Linux x86_64, Intel Xeon, two logical CPUs,
Rust 1.98.0, optimized release. Raw JSON and environment are in
[benchmarks/logos/](benchmarks/logos/). This is one shared runner, not a fleet
or statistical confidence interval. Re-run on target hardware before decisions.

## Raw lexer results

Milliseconds, native / Logos. Native raw snapshots include their native fold
pass. Logos cache measurements exclude deferred native folding and framework
overlays, so these are component costs, not an equivalent completed UI workload.
Framework cases here measure the base lexer on framework-shaped text.
Native edit is the current full-snapshot path, not an incremental Lexilla API.
Logos edit uses an unlimited cache allowance to characterize its algorithm;
production defers invalidations above 64 KiB to a cooperative worker instead.
Fresh caches are prepared outside each edit timer. Initial timings alternate
backend order. Destruction is included for raw initial/native edit timings.

| MiB class | Case | Initial native / Logos | Tail edit native / Logos | Front Logos | Middle Logos |
|---|---|---:|---:|---:|---:|
| 1 | php | 10.21 / 23.51 | 10.18 / 1.92 | 24.05 | 13.07 |
| 1 | mixed | 9.22 / 24.43 | 8.92 / 1.73 | 20.52 | 11.06 |
| 1 | blade | 12.56 / 7.13 | 13.12 / 1.48 | 7.52 | 4.54 |
| 1 | laravel | 15.72 / 85.80 | 16.52 / 1.17 | 86.60 | 44.33 |
| 1 | symfony | 13.53 / 74.18 | 14.37 / 1.11 | 75.19 | 38.92 |
| 5 | php | 46.46 / 114.74 | 52.00 / 11.32 | 121.21 | 65.45 |
| 5 | mixed | 44.16 / 101.55 | 45.31 / 10.09 | 102.01 | 54.91 |
| 5 | blade | 60.58 / 35.61 | 65.74 / 8.28 | 38.78 | 23.51 |
| 5 | laravel | 79.58 / 437.18 | 85.21 / 7.53 | 434.75 | 237.16 |
| 5 | symfony | 70.01 / 388.19 | 73.89 / 6.39 | 372.42 | 190.62 |

## Production Syntax::parse worker path

Includes Rope conversion, snapshot import and applicable framework overlays.
The feature flag is selected before process launch. No project filesystem scan
is included: profiles are explicit. Logos folding is measured separately after
initial parsing; the production 250 ms debounce is not included. Native parse
already includes its folding. These are worker latency proxies, **not measured
input-to-paint latency, frame drops or UI thread stalls**. No UI win is claimed.

| MiB class | Case | Initial native / Logos | Tail native / Logos | Extra Logos fold refresh |
|---|---|---:|---:|---:|
| 1 | php | 14.90 / 40.53 | 14.88 / 3.09 | 13.91 |
| 1 | mixed | 22.57 / 36.66 | 22.52 / 3.33 | 21.63 |
| 1 | blade | 16.38 / 39.80 | 16.36 / 25.45 | 15.58 |
| 1 | laravel | 26.13 / 150.29 | 26.07 / 51.79 | 28.92 |
| 1 | symfony | 24.35 / 93.36 | 24.47 / 6.77 | 23.46 |
| 5 | php | 77.32 / 177.46 | 78.52 / 24.28 | 74.28 |
| 5 | mixed | 117.32 / 164.05 | 118.55 / 25.58 | 113.14 |
| 5 | blade | 84.75 / 228.13 | 84.94 / 150.78 | 85.02 |
| 5 | laravel | 132.56 / 758.17 | 133.27 / 266.86 | 134.21 |
| 5 | symfony | 132.68 / 476.33 | 125.10 / 50.06 | 124.32 |

## Memory and responsiveness

Each RSS result launches a fresh process for exactly one case and backend.
The baseline includes all generated inputs and process/runtime initialization. Delta is retained process RSS while one
result is alive, not allocator bytes or peak memory. It includes each backend's
source/result ownership costs, and is noisy at page granularity. It excludes
production framework overlays and UI state. JSON stores both absolute readings.

| 5 MiB class | Native RSS delta MiB | Logos RSS delta MiB |
|---|---:|---:|
| php | 14.47 | 13.12 |
| mixed | 14.47 | 16.99 |
| blade | 16.33 | 16.07 |
| laravel | 16.53 | 11.06 |
| symfony | 16.02 | 6.73 |

Across this run the longest observed 16 KiB cooperative worker step was 10.84 ms; the longest observed cancellation acknowledgement after one yield was 0.04 ms. These are observations, not deadlines or guarantees. The counters bound scan work, not allocation, OS scheduling or native folding. Native folding is still uninterruptible.

## Accuracy and acceptance

Every measured cache edit and cooperative result is asserted byte-for-byte
against full Logos lexing. This catches incremental inconsistency, not shared
lexer errors. Native scope agreement is recorded only as a disagreement
finder: mixed hosts and framework conventions intentionally differ. It is not
an accuracy percentage. Independent PHP `token_get_all` checks, modern syntax,
multiline, incomplete and framework fixtures remain the accuracy acceptance
gate from [logos-corpus.md](logos-corpus.md). The oracle tokenizes strings;
it never includes or evaluates the fixture. CI requires it.

## Why the gate stays closed

Tail reuse helps plain PHP and mixed documents, but initial Logos cost is higher
in the production path. Laravel/Blade overlays can outweigh raw lexer savings,
and deferred folds add work. Memory results vary by corpus. This does not justify
a default switch. Before reconsidering, measure real input-to-paint and frame
latency under rapid edits on target desktop hardware, compare complete folds
and overlays, use representative user files and repeated isolated runs. Neither
raw Blade speed nor one runner's cooperative step time is sufficient evidence.
