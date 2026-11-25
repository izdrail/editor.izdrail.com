# Logos lexer for Gravity Editor: feasibility report and plan

Status: implementation in progress. This document is a technical work register, not an authorization record.

## Current architecture (read from the repo)
- Highlighting is Lexilla (C++ lexers statically linked by `lexilla-rs`). The safe Rust `Snapshot` API (`lex`, `runs`, `overlay`, embed ranges, fold levels) returns one u8 style per byte. Tree-sitter is not used for painting.
- Custom C++ lexers: `lexilla-rs/vendor/lexilla/lexers/LexPHP.cxx` and `LexLaravel.cxx` (see `docs/php-lexers.md`).
- Routing: `lapce-core/src/lexilla.rs`. A `.php` file starting with `<?php` and with no inline HTML after a close tag uses `php`, or `laravel` in a Laravel project (artisan, or composer.json with laravel/framework). Mixed or HTML-first files use `hypertext`. Twig and JSX/TSX use Rust passes in `lapce-core/src/web_templates.rs`. `LapceLanguage::Blade` exists in `language.rs`.
- Documented limits: nested `${...}` / `{$expr}` interpolation is partial; standalone lexers colour inline HTML as one style, so Blade directives only show in `.php` files that start with `<?php`; word lists are a starting set.
- A Rust integration boundary exists (`lapce-core` calls `lexilla_rs::Snapshot` in process). A large-file benchmark harness exists: `lapce-core/examples/snapshot_perf.rs`.

## Logos
Current version 0.16.1 (https://crates.io/crates/logos, https://docs.rs/crate/logos/latest). Pure Rust, no runtime dependencies. It is a regular-language lexer, so heredoc/nowdoc closing labels, nested interpolation and HTML/PHP/Blade mode switching need a small hand-written driver with one Logos enum per mode (switching with `morph`).

## Integration options
1. Native in-process Rust lexer: recommended. Simplest, no new process.
2. Rust language service: extra process, no gain for a lexer.
3. WASM in the editor: the editor is already Rust, so this only adds a boundary.
4. Rust tokenise plus existing renderer: this is option 1, since rendering already consumes style bytes and theme scopes.

## Recommendation
New workspace crate (working name `gravity-lex`) with Logos mode enums. Output the same `Snapshot` style IDs and scope table, so themes, folding, pairing, completion and diagnostics keep working. Route to it behind a setting or env flag. Lexilla stays the default. Make it the default only if benchmarks and an accuracy corpus show a measurable win. Incremental re-lex from the nearest safe line start with per-line mode-stack checkpoints. Run off the UI thread. Never execute PHP; treat input as untrusted; no panics on malformed input.

## Risks
- Lexilla PHP already covers most listed syntax. Real expected wins: Blade and HTML in one pass, interpolation accuracy, incremental re-lex, Symfony/Doctrine attribute awareness. Raw speed gain is unproven.
- u8 style budget may need new IDs for Blade and Symfony scopes.
- Keep the change to a new crate plus one flagged routing hook to avoid conflicts with the editor UI lane.

## Phases
L1 skeleton and PHP tokens, L2 PHP syntax depth, L3 HTML and mode driver, L4 Blade, L5 Laravel overlay, L6 Symfony overlay, L7 routing hook behind flag, L8 incremental and off-thread, L9 test corpus, L10 benchmarks, L11 docs. Order: L1, L2, L3, L7, L9, L10 first (gate: does it beat Lexilla), then L4, L5, L6, L8, L11.

## Approved implementation scope

25 November2026: initial L1-L3 prototype checkpoint. Lexilla remains default. Initial Rust PHP/HTML tokens are lexical, not semantic PHP
analysis. Accuracy/performance and framework overlays remain separate gates.

## L1-L3 implementation checkpoint

The `gravity-lex` crate pins Logos 0.16.1 (MIT OR Apache-2.0, minimum Rust
1.80, compatible with this workspace's Rust1.98). Source/API inspected from
https://crates.io/api/v1/crates/logos/0.16.1/download before implementation.
The crate uses PHP and HTML Logos enums and a bounded hand driver for quoted
strings, interpolation, comments and heredoc/nowdoc. Returned byte styles reuse
LexPHP IDs0-21. Safe import validates length/IDs and reuses the adapter's scope
mapping. Checkpoints currently record only safe HTML/PHP transitions; they do
not implement incremental invalidation, reuse or folding.

Build the experimental routing path with `lapce-core` feature `logos-php`.
There is one hook, for PHP only and excluding detected Laravel/Symfony/WordPress projects.
Default builds, Blade, Twig and framework-aware native routing remain unchanged.
No UI setting or persisted configuration was added. This opt-in prototype does
not preserve native folding levels or embedded CSS/JS overlays and must not be
promoted to default on these tests alone.

Current tests cover modern lexical words, UTF-8 variables/offsets, numeric forms,
PHP tags/mixed HTML, comments, quotes, braced/simple interpolation, indented
heredoc/nowdoc and incomplete typing prefixes. Whole interpolation expressions
share the existing variable style; their internal semantic tokens are not
parsed. PHPDoc remains documentation-comment style, not annotation semantics.
Context-sensitive names/types, malformed heredoc indentation diagnostics,
framework overlays, full accuracy corpora, incremental reuse, parity and all
performance claims remain separate open gates. L1-L3 are an initial implementation,
not a claim of exhaustive PHP grammar coverage or a completed engine replacement.


Hosted checkpoint cd579c9: feature-enabled import routing/scope test and full
Rust workspace pass in run45577917298. Seven focused crate tests and native
style-scope parity pass locally after751a0aa. No throughput, incremental,
memory, GUI responsiveness or highlighting-accuracy win has been measured.
https://github.com/izdrail/editor.izdrail.com/actions/runs/45577917298


## Issue6, L1 completion gate

L1 is limited to the crate skeleton, basic lexical tokens and style/byte contract.
The integration suite compares every public style ID0-21 against the native
LexPHP definitions, checks basic token categories and visits every Unicode scalar
under the public UTF-8 input contract. Existing malformed-prefix/noise tests
check termination and byte alignment across mixed strings. Full PHP depth,
HTML mode coverage, framework overlays and performance are later issues.
No claim of universal panic-freedom is inferred from a finite test suite.
