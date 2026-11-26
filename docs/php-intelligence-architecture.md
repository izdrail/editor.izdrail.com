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

Editor integration uses the mixed PHP grammar for all PHP documents in Phase 1,
including pure PHP files with opening tags. The library exposes php_only for
explicit tagless inputs; automatic editor grammar selection is not implemented.
Paint snapshots and folds publish before structural parsing; the separate context
publication is cancellation- and revision-gated. Non-cancellation incremental
errors log at debug level and retry a full parse.

## Phase 2 implementation plan

The existing lapce-index Tantivy store becomes proxy-owned. Composer maps and
hot PHP symbols live beside that same store. App first-open writer is removed;
no duplicate index and no new language server. Root and installed package
Composer JSON are read as bounded data, never executed. Reuse existing serde,
serde_json, ignore, sha2 and gravity-lex (Logos) dependencies, all already in the
workspace lockfile. The worker receives existing notify watcher events, batches
changes and keeps the last committed reader available during extraction.
Cache version/config fingerprint and per-file content hash guard reuse; replace
and stale-delete use the same Tantivy writer. Atomic hot-cache persistence occurs
after commit; failed/cancelled passes must not publish a new hot generation.
The paint and context revisions are tracked independently: php_context_rev
identifies the tree's actual source revision, not the latest painted revision.
Consumers must use context only when that revision matches the document. A
cancelled structural job preserves the older context for safe future fallback.
Markdown code-block highlighting uses the paint-only entry point as well.

### Phase 2 scope and measured baseline

The old app cache path (DefaultHasher of workspace Path) is preserved for store
compatibility. Proxy tabs share an in-process owner; another process receives the
existing Tantivy writer-lock error instead of creating a competing writer. Cache
version 1 is a hot-symbol sidecar, not a second search database. Schema errors are
not worked around by deleting data. The app no longer starts its old worker.

Composer root/autoload-dev mappings, directory arrays, longest-prefix-first plus
fallback, classmap/files, exclusions and installed vendor manifests are read.
composer.lock participates in invalidation; PHP/autoload scripts are never run.
Explicit vendor autoload roots override the common vendor gitignore. Application
roots retain ignore rules. Symlinks are not traversed, files cap at 1 MiB,
Composer metadata at 8 MiB, cache at 64 MiB. Declared external autoload roots are
read on reconcile, but the existing workspace watcher only automatically observes
paths inside its workspace. External-root live tracking is a known limitation.

Extraction reuses gravity-lex Logos byte classes and the pinned PHP AST. Namespace,
class/interface/trait/enum/function/method/property/constant/attribute/import symbols
carry qualified names, scope owner, visibility, aliases and byte positions. This
is declaration discovery, not PHP semantic/type resolution. Attribute text is
stored but not resolved. Malformed trees recover identifiable declarations;
recursion is bounded. Promoted constructor properties and trait adaptations are
not yet modeled as full semantic members.

Incremental updates still traverse metadata, but hash/parse only touched or changed
files, replacing each path's records and deleting stale paths in one Tantivy
commit. An all-file cache rebuild stages extraction before clearing. Existing
readers remain usable. Hot prefix queries are background RPC work with a bounded
result count, currently linear over cached symbols, not a completion-ranking or
Nucleo replacement. A failed commit rolls back queued changes; a stale cache
reconciles from source on next pass. Watcher events debounce for 120 ms, preserve
events arriving during a pass and handle both rename paths. Superseding file
changes queue the next incremental pass; shutdown cancels between files. Parsing
an individual bounded file is not preempted by shutdown.

Reproducible baseline: `cargo run --locked -p lapce-index --example
php_project_perf -- 3000`, debug build on the same two-logical-CPU Linux worker.
3,000 synthetic PHP classes plus composer.json: initial 1,636 ms, warm 194 ms,
one-file change 220 ms, 1,000 prefix queries 657 ms, peak RSS 77,496 KiB. Parsed
files initial/warm/edit = 3001/0/1. This is a single development baseline, not a
release benchmark or UI-latency claim; Phase 6 must collect repeated release
measurements and real-project fixtures. The benchmark prints/keeps its fixture.

Phase 2 review corrections: all declared Composer roots/manifests are normalized
and canonicalized beneath the workspace. Absolute and escaping paths warn and
are skipped; symlinked vendor packages outside it are not read. This intentionally
does not support external path repositories without a future explicit grant.
The existing workspace is the read boundary. Classmap exclusions apply only to
classmap-only paths, not PSR-4/autoload-dev/files paths. Watcher gates discard
ignored/generated/unsupported/oversize/binary changes before enqueueing work.
Writer open retries for 3 seconds to cover a cancelling previous owner; spawn
failure publishes failed state. Import and attribute metadata are not SymbolDef
records. Anonymous-class members are omitted until an anonymous-owner model is
implemented. Hot queries remain linear and should become indexed before Phase3.

Ownership migration inventory: lapce-app project_index is referenced only by its
module declaration, WindowTabData optional field, and old start initialization.
There are no callers of ProjectIndexHandle::index() and no app symbol_prefix()
consumer. Existing file search and LSP workspace-symbol/completion paths do not
use this handle, so disabling its writer changes no connected provider. The new
PhpIndexQuery RPC is the Phase3 connection point, not a claim that those providers
already consume it. Keep native/LSP provider routing for Phase3.

The marker prevents reuse after process-crash partial publication. The sidecar is
written with file fsync and atomic replacement; directory fsync is not currently
performed, so no power-loss durability claim. A marker left after a failed commit
costs one full reconciliation. Missing/old/invalid sidecar starts a staged rebuild
that clears the existing records only when its extracted replacement is ready to
commit, not at open time. Empty unchanged passes skip marker/commit/persist.

Review boundary: Composer packages pointing outside the workspace, including
outside path repositories, are deliberately no longer indexed. Internal vendor
symlinks resolving inside the workspace remain supported. Events normalize both
workspace symlinks and removed-path parents before containment checks. Removed
extensionless paths are treated as potential directories so stale descendants
are reconciled. Cache path hashing uses the original workspace path for migration
compatibility, while worker identity/path checks use the canonical root.

64 MiB sidecar cap is enforced before mutating Tantivy. If a project exceeds it,
the pass fails with an explicit state message and retains the previous committed
generation; it does not write a sidecar it cannot load. A chunked sidecar is a
future large-project improvement. This means projects beyond the cap are not yet
supported in Phase2 rather than silently rebuilding on every launch.

Consumer audit command (all workspace crate directories):
`rg -n '\.project_index|ProjectIndexHandle|project_index::' --glob '*.rs' lapce-*`.
Only the dormant worker definition and WindowTabData type remain. The old module
is marked legacy/dormant and must not start while proxy owns the writer.

Phase 2 merge review approved with follow-ups tracked in #19/#17. The current
64 MiB behavior fails safely with prior generation retained; before claiming huge
monorepo support, P6 at latest must commit Tantivy/publish hot data and skip the
oversize sidecar with a status warning instead. Other open optimization work:
dirty-only passes, less copying via Arc, a query worker instead of thread/query,
heavy watcher filtering off the callback/composer mutex, richer RPC/pipeline
coverage, and repeated 20k-50k release vendor-like benchmarks. Configuration-only
edits already hash-reuse unchanged files but still read them to verify the hash.
All baseline numbers above are one debug run, not release targets.

## Phase 3 plan

Keep the current CompletionData popup and Nucleo matcher. Add a separate native
batch on the same request id/path/revision, not input_items (LSP-owned metadata).
Context buckets precede fuzzy score only for PHP, preserving other languages'
source ordering. The app reads current_php_context when available; otherwise a
bounded last-256-byte trigger classifier starts a native request, never a UI parse.
Native queries use the proxy's existing hot snapshot through asynchronous RPC.
Merge de-duplicates across sources in favor of LSP, retaining server docs/edits.
Methods/properties are offered only with a known receiver owner, initially $this
or explicit static class names. Unknown dynamic receivers stay LSP-owned rather
than displaying every project's member. Native namespace/class/function symbols
stay available when no LSP is running. Immediate triggers include bare $, ->, ::,
new/use space and namespace separator. Projection paths keep their guarded source
mapping and are not silently extended by this first native adapter. Scope/type
inference remains Phase4; Phase3 uses declaration context only.

Phase3 uses absolute qualified insertion rather than adding automatic use edits.
This avoids import collisions and requires no filesystem edit beyond the current
completion. Unicode/FQCN replacement retains byte offsets. Native items have no
resolve data and are excluded from server completionItem/resolve. Parser catchup
refreshes open sessions; until a matching current AST exists no native symbols are
shown, avoiding false triggers in embedded languages/comments/strings.35ms timers
coalesce obsolete native requests by generation and revision. Server filters
kind/owner/static/access before result cap; hot query is still linear in symbols.
Class parents and trait declarations supply inherited members; dynamic receiver
types remain Phase4. Local variables are declaration-only and bounded, not a full
PHP scope/type engine. Unsaved top-level declarations join disk candidates.
Review correction: a non-PHP region disables only native PHP rows and triggers;
it does not cancel existing Laravel quoted project strings, snippets, buffer
words or LSP requests. Context catchup preserves explicit Ctrl+Space sessions.
Chained dynamic receivers are unknown, not treated as the current class. Current
unsaved class members join the native batch when a declaration tree is recoverable;
malformed member shapes still fall back to LSP. Native namespace duplicates use
qualified insertion identity, retaining two same-named classes from different
namespaces. Configured LSP documentation wins unambiguous duplicates.

Phase3 reproducible adapter baseline: ignored test `native_50k_adapter_baseline`
constructs50,000 vendor-like class candidates with no LSP and repeats5 times.
Debug isolated adapter min41.892ms, median51.349ms on the two-logical-CPU worker.
This intentionally includes candidate cloning but excludes index lookup, RPC,
Nucleo/UI and rendering. It is not end-to-end completion latency. A separate
ignored proxy query fixture `native_50k_proxy_baseline` measures the bounded hot
query; run it after linking the proxy tests. No release/UI claim until measured.


## Phase 4 plan and draft scope

The existing PHP Document owns a cached scope arena produced on the syntax
worker from its existing AST and Rope chunks. Paint publishes first, Phase 1
context second, and scope analysis third. Analysis readiness triggers a separate
completion refresh. The existing same-session CompletionData input-key cache
prevents duplicate LSP requests for already-requested inputs; the analysis refresh
uses that existing path rather than a separate LSP request. No new dependency, parser, runtime,
store or language server. Byte ranges remain the source coordinate system.
Analysis checks cancellation, caps AST traversal and diagnostics, and reuses
unchanged function snapshots by range and content fingerprint. Offset-shifted
functions are rebuilt, not reused with stale absolute ranges.

Parameters, straight-line assignments, direct object construction and variable
copies carry declared, documented or inferred confidence. PHPDoc @param, @var
and @return support single named types; unions, intersections and generics remain
unknown rather than choosing one owner. Explicit closure captures and arrow free
variables use the lexical parent at creation; ordinary nested function parameters
do not inherit a same-named outer variable. Completion resolves typed receivers
through the existing namespace/import map and Phase 3 query, preserving LSP rows.

Diagnostics use the existing Problems and inline diagnostic channel. Native rows
have source native-php and are replaced separately; server rows overlapping the same lines win, including wider server ranges.
Native diagnostics are excluded from code-action requests to language servers.
Edits clear native rows before applying the normal server span delta; the full
span rebuild occurs only when native rows existed, not every keystroke. Undefined/unused locals and simple post-return unreachable statements are
limited to parse-clean, straight-line scopes. Dynamic variables, includes,
extract/compact, control flow, nested scopes and other uncertain constructs
suppress native diagnostics. Type invalidation is separate: out-params and
uncertain writes clear only affected bindings; arbitrary dynamic-local writes
clear known types after that statement. Unrelated types survive control flow.
Assignments become available after their RHS, not on their LHS. Missing
diagnostics are preferable to false alarms.
Types and diagnostics are bounded local hints, not whole-program PHP semantics.
The draft remains uncompiled locally due disk constraints; hosted CI and targeted
GUI fixtures are required after reviewer approval and main push.

The 5k-line scope benchmark includes parse, context publication ownership and
Arc::make_mut Document cloning before analysis, matching the split worker path.
Loop writes invalidate their targets from the outermost loop entry to avoid
carrying a pre-loop type into a later iteration. Implicit foreach/catch/global/
static and destructuring bindings also invalidate old types.

The editor completion effect tracks (php_context_rev, analysis_ready). Its second
publication calls update_completion(true); CompletionData::request registers the
input key before the proxy call and the same-session path checks that key.


## Phase 5: Laravel bridge (issue 22)

Composer requirement/lock/installed-package evidence enables Laravel enrichment
inside the existing bounded PHP parse pass. An artisan filename alone is not
proof. Cache version 3 invalidates old metadata; composer fingerprint changes
reparse files, and edit/delete reconciliation removes stale aliases. No PHP,
Artisan, service provider or user application is executed.

The one symbol graph now records directly evidenced model, facade, provider,
request, command, queued-job and collection roles. Eloquent legacy scopeX methods
and exactly resolved Laravel Scope attributes produce public callable aliases.
Relationship methods are tagged only for direct return $this->knownRelation(...).
Custom facade @method static declarations become native members. All retain
real byte locations and participate in the existing completion RPC and single
Tantivy writer/cache, including Composer roots outside app/Models. Exact indexed
Laravel static-call definitions resolve to the source method/comment, with an
origin/cursor/revision guard; ambiguous or absent targets keep existing navigation.
The index worker does the parse/enrichment; UI lookup uses the cached context.
50,000 nodes, 4,096 aliases, file size and cancellation bounds remain explicit.

### Existing providers retained, not reimplemented

- Bundled facades: laravel_completion.rs, sixteen facade packs, namespace/import
  validation, existing public API signature baselines.
- Models/relationships and legacy scope navigation: model_index.rs,
  eloquent_scopes.rs; the new aliases also work outside conventional folders.
- Routes/middleware, bindings, commands, events/listeners/jobs, request fields,
  config, helpers, Blade and collection/query-builder completions remain in the
  existing project_completion and dedicated navigation providers. Their tests
  run unchanged in the workspace suite; this is reuse, not a second PHP index.

### Deliberate limits

Roles require a direct canonical framework ancestor/interface in this pass;
project-base-class role propagation, dynamic facade accessor execution, container
runtime bindings, macros, relationship result types, schema introspection and
chained builder type propagation are not inferred. Existing string providers
remain bounded text/metadata providers, not whole-framework semantic resolution.
Facade PHPDoc targets open the declared contract, not an invented backing method.
No claim that a queued class is necessarily a job rather than a queued listener.
Tests cover Composer gates, scope forms/access/collisions, false attributes,
facade contract source ranges, direct roles, edit and Composer invalidation,
unsaved adapter precedence, plus no-LSP GUI54 completion and F12 navigation.

Review notes: native static F12 is gated by Laravel Composer package evidence;
non-Laravel calls use the previous path without the new RPC. It reads one rope
line, not the document. Cached completion_allowed rejects comments/strings.
Unsaved *new* scopeX methods do not gain a callable alias until saved/indexed.
An already indexed attribute scope alias supersedes its unsaved protected original
for completion, even inside its declaring class. Facade pack/native member rows
share the label/insertion key and are deduplicated by existing source priority
(test: facade_pack_and_native_method_dedupe). Routes/config/Blade listed above
are retained earlier providers, not new Phase 5 indexing.

Laravel F12 gating is a workspace signal published by the proxy from the same
Composer instance as the indexer (manifest, lock and installed.json evidence),
refreshed on Composer watcher changes. F12 performs no project-profile file IO.
Shared workspace owners notify each attached UI using weak, identity-deduped
observers pruned when dispatchers close; initial state is false until
background workspace initialization sends the evidence.

## Phase 6: stabilisation and measured optimisation (issue 23)

Audit: the three ScopeUser rows seen in GUI54e were same-FQN declarations in
three earlier on-disk fixtures, not a cache failing to delete an old file. Each
inserts the same absolute name. Generic PHP snippets also repeated identical
actions because the prior same-tier dedupe only collapsed snippets for Blade.
P6 collapses identical Native/Snippet actions by source, label, insertion, text
edit, additional edits and insertion format. Distinct namespace insertions,
auto-import edits and LSP overloads are preserved. F12 still refuses ambiguous
source targets; hiding identical completion actions does not pick a definition.

The proxy's prefilter keeps Unicode fallback but uses ASCII byte matching for
ordinary PHP names. Ranking lowercases each matched name once and selects top K
before sorting only K candidates, rather than sorting all N. Order is the same
starts-with, name length, lowercase name order, with path/byte tie-breakers.
No second fuzzy engine: Nucleo still owns final app matching and ranking.

Reproduce hosted debug benchmarks with scripts/test-linux-hosted.sh. The ignored
native_2000_file_index_latency_and_rss test creates 2,000 PSR-4 PHP files (10,000
class/method symbols), measures initial index readiness, one-file reconciliation,
Linux process RSS before/after, and concurrent cached query p95 while the worker
reconciles. It asserts all 2,000 classes and the edited replacement appear. At least 200 queries are sampled around reconciliation. The
RSS is indicative only, whole test-process memory, not an allocator attribution or peak. Cold
queries may be empty while the first snapshot is built; later reconciliation
retains usable symbols. Query samples include lock contention, not RPC/render.
GUI55 injects 2,000 files through the actual watcher path and verifies typed text
and capture survive. Its timer includes xdotool plus a deliberate clipboard wait,
not a frame-time promise; pixel inspection remains a separate gate.

P5 accepted baseline, run45796880347 (aaeda7c),
https://github.com/izdrail/editor.izdrail.com/actions/runs/45796880347 : 50k debug min/median ms proxy
Model49 243.734/244.011, empty 20.547/21.013, depth64 45.354/45.368, adapter
49.374/49.631. Scope5k parse/publication-clone/analysis 223.107/225.214.
Runner load varies; compare the next hosted results only with these caveats.
Final P6 measurements are recorded with the acceptance run, not guessed here.

P6 adds no dependencies. Source inventory: completion.rs (local action dedupe,
server fallback/recovery assertions), proxy php/mod.rs (bounded top-K and large
fixture benchmark), lsp.rs/mock_server.py (crash/reopen regression), registry.rs
(disabled configuration-only Intelephense preset), hosted test/GUI scripts and
these architecture/LSP docs. Earlier phase files are documented above and in
phase-specific commits. Remaining limits include linear candidate scanning,
whole-snapshot clone/publication, no full PHP control-flow/generic inference,
no dynamic framework execution, no proprietary-LSP runtime validation and no
claim of constant-time indexing or guaranteed frame deadlines.
