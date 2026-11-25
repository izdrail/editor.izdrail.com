# Native PHP intelligence architecture, Phase 1

## Assessment of the current fork

Gravity is a Lapce 0.4.6-derived workspace on Rust 1.98. Painting now defaults
to Logos for supported PHP/Blade routes, with Lexilla available in Settings.
Neither painting path is a semantic parser. Existing components remain:

- `lapce-xi-rope` owns editing, deltas and undo. `SyntaxEdit` maps edits to
  tree-sitter byte coordinates. Phase 1 corrects its line/UTF-8 column conversion.
- Floem owns rendering; `Doc::trigger_syntax_change` already dispatches syntax
  work to rayon and accepts only the matching revision/cancellation generation.
- `lapce-index` already uses tree-sitter 0.22.6 and pinned PHP grammar 0.23.11
  for structural definitions, plus one Tantivy project store and ignore walker.
  The first-open store owner is currently `lapce-app/src/project_index.rs`, not
  `lapce-proxy`. Phase 2 must reuse this store rather than create a second one.
- `lapce-proxy` owns document buffers, filesystem watcher and LSP routing.
  Completion uses the current controller and nucleo. Native completion is not
  a replacement LSP. Phpactor/Intelephense remain the deeper semantic layer.

## Phase 1 boundary

`lapce-php` is a pure structural context crate shared by core and index. It
centralizes the existing PHP grammar ABI bridge, supporting mixed PHP/HTML and
`LANGUAGE_PHP_ONLY` without a second native tree-sitter runtime. Dependencies
reuse the exact existing tree-sitter 0.22.6, grammar 0.23.11 and xi-rope versions.
No new external dependency/version. Rust workspace code is Apache-2.0;
tree-sitter and tree-sitter-php are MIT; lapce-xi-rope is Apache-2.0. Existing
Logos (MIT/Apache-2.0), Tantivy (MIT) and ignore (MIT/Unlicense) stay in their
current components. No PHP code is executed or third-party server distributed.

`Document` holds an immutable Tree/Rope publication, disjoint sorted region
intervals and per-top-level-unit cached regions. Queries use binary search,
O(log N), without reparsing or converting the whole Rope. Cursor classification
uses the cached AST and at most a 256-byte prefix. The parser reads Rope chunks
through tree-sitter's callback and receives the current job cancellation flag.

`Syntax::parse` maintains this document in its existing background job,
independently of the selected painter. Supplied sequential InputEdits edit the
previous tree before incremental parse. Affected top-level units are queried
again and their intervals replaced; unaffected units shift/reuse intervals.
Syntax-boundary changes may invalidate a whole enclosing unit. Region
publication still scans/splices top-level metadata, not a constant-time update.
Missing edit history or a skipped buffer revision falls back to full parse, never stale regions. Host checkpoints preserve script/style and quoted-attribute context across PHP islands.

Tree-sitter queries identify HTML text, PHP comments, strings and heredoc/nowdoc
nodes. PHPDoc is distinguished by its opener; SQL labels and conservative
SQL-leading strings create SQL regions. Script/style host scanning identifies
JS/CSS bodies. Malformed/incomplete syntax retains a usable tree. Region data
is separate from painting scopes and does not alter formatting/diagnostic or
completion ownership in Phase 1. Later phases consume it for provider routing.
Existing comment/bracket rules remain on validated painter snapshots until
those consumers can safely use a revision-matched context result.

## Verification and limits

Run `cargo test -p lapce-php` and the core/index suites. Tests cover mixed and
pure PHP, SQL strings/heredoc/nowdoc, PHPDoc, JS/CSS, malformed prefixes, UTF-8
byte/point roundtrips, full-vs-incremental edits and cancellation. Reproduce
component timings with `cargo run --release -p lapce-php --example context_perf`.
This avoids adding criterion solely for one benchmark. Results are not UI
frame/input latency. A large enclosing function may still need a full unit scan.
SQL detection is a lexical convention, not query parsing or inferred PDO types.
No Phase 3 completion ranking or Phase 4 semantic type inference is claimed.

## Next integration

Phase 2 adds Composer/PSR-4 and richer symbol records to this shared structural
layer and the existing store, moves indexing execution behind the existing
proxy worker/watcher boundary without duplicate owners, and retains readable
committed snapshots while updates continue. It requires its own tests/review.

## Recorded Phase 1 component benchmark

2026-11-25, Linux x86_64, two logical Xeon CPUs, Rust 1.98 release,
five iterations: 1,428,896 bytes, 80,001 regions. Initial parse min/median
259.497/262.377 ms; one million lookups 26.281/26.330 ms; middle identifier edit
40.129/43.020 ms with one unit rescanned. The update still shifts/splices
O(units) metadata. P6 owns reducing that cost; lookup itself stays O(log N).
Reproduce with `examples/context_perf.rs`. No UI or indexing claim follows.

SQL limitations: detection uses a short leading-keyword list, and only an
exact case-insensitive SQL heredoc/nowdoc label. Interpolated strings can split
SQL text across content nodes: only a segment beginning with a recognized
keyword is marked, while PHP interpolation remains PHP. PHPDoc is currently
one whole-comment region; annotation/type parsing belongs to P4 (#21).
Comment/bracket/formatting/diagnostic consumers remain painter/LSP based;
context-driven provider integration belongs to P3/P6.
