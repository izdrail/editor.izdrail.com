# Project index (lapce-index)

The current foundation creates one embedded [tantivy](https://github.com/quickwit-oss/tantivy)
index per local workspace tab. It is intended to supply file search,
workspace-symbol search, and the local-first completion cascade
(local index -> attached LSP server -> llm.izdrail.com fallback).
Research and the tool comparison that chose tantivy:
[research/editor-code-indexing/01-code-indexing-tools.md](https://github.com/izdrail/research.izdrail.com/blob/main/research/editor-code-indexing/01-code-indexing-tools.md).

## Schema

Every record carries the same fields; `symbol` is absent on plain file records.

| Field    | Type          | Indexed | Stored | Meaning |
|----------|---------------|---------|--------|---------|
| path     | string (raw)  | yes     | yes    | Workspace-relative file path; exact-match key for deletes. |
| kind     | string (raw)  | yes     | yes    | `file`, `symbol-def` or `symbol-ref`. |
| language | string (raw)  | yes     | yes    | Language id (`php`, `javascript`, `typescript`, `vue`, `liquid`, ...). |
| symbol   | text          | yes     | yes    | Symbol name, when the record is a symbol. |
| line     | u64           | yes     | yes    | 1-based line for symbols; 0 for file records. |
| col      | u64           | yes     | yes    | 0-based column for symbols; 0 for file records. |
| body     | text          | yes     | yes    | File text, or the symbol's source line. |

Queries run over `symbol` and `body` with BM25 ranking
(`QueryParser::for_index(&index, vec![symbol, body])`).

## Lifecycle

- `ProjectIndex::open_or_create(dir)` opens or creates the on-disk index.
  An existing metadata file must open successfully with the exact expected
  schema. Corrupt metadata or a different schema returns an error without
  overwriting the cache. The worker publishes a Failed status and leaves the index unavailable for
  that tab. Automatic cache migration/rebuild is not implemented yet.
- `add_entry` queues a record; `remove_path` queues deletion of every record
  for one path (used on save: delete, then re-add that file's records);
  `commit` flushes and explicitly reloads the reader, so searches made
  immediately afterward see the change without waiting for a watcher.
- `ProjectIndex::in_memory()` is the volatile variant for tests.

## Build plan status

1. (this patch) Crate skeleton: schema, add/remove/commit/search, unit tests.
2. (item 243) Walker: `walker::index_workspace` walks the workspace
   (respecting .gitignore, hidden files, and a 1 MiB size cap; skips binary
   files), indexes eligible text files, commits in batches of 50, and reports
   progress/cancellation. Each pass is a full rebuild, not incremental:
   it clears previous records before walking, avoiding duplicates and stale
   entries for deleted or newly ignored files. Pre-cancelled passes leave
   the existing cache intact; cancellation mid-pass leaves a partial index.
   Item 244 starts a named background worker from local workspace tabs.
   Repeated progress and final status cross an mpsc channel to a UI effect.
   A scope-owned drop guard cancels the walk on tab disposal. Item 248 moves directory access, index opening, schema validation and writer
   construction off the UI thread as well. The handle starts at Opening;
   index() returns None until the worker publishes it, then returns an Arc
   without blocking. Worker open errors become Failed. Cancellation is checked
   before opening and again before publication. Status-bar display and
   save-time updates remain pending.
3. tree-sitter symbol extraction per saved file (delete-by-path, re-add).
4. Completion cascade: local index -> LSP -> llm.izdrail.com.
5. Per-framework LSP presets (PHP/Magento/WordPress/Symfony, TS/React/Angular,
   Vue via Volar, Liquid via Shopify theme-check).
6. MCP presets and ACP docs for Claude/Codex/opencode.

## Read limits

Item 249 enforces the 1 MiB cap on actual bytes read as well as metadata.
A file that grows after its metadata check is skipped once the bounded reader
sees one extra byte; it is never read to the end without a bound. Read failures
also count as skipped files. This bounds per-file input allocation, not total
index size or total process memory. Ignore/hidden filters and the cap mean
this is not literal all-file coverage; coverage controls remain future work.

## Symbol prefix candidates (item 250)

`symbol_prefix(prefix, language, limit)` searches symbol-definition records
only, scoped to an exact language ID, with case-insensitive token-prefix
matching. It accepts identifier letters/digits/underscore, rejects query
operators, caps the prefix at 128 UTF-8 bytes and results at 200 records.
These are lexical candidates, not resolved types or full-name namespace
matching. Names pass through Tantivy's default TEXT tokenizer; long tokens
and punctuation follow that tokenizer's rules. Ranking/ties are not a stable
completion order. A zero limit returns no candidates.

This API is preparation only: the walker now emits JavaScript definitions (item 251), but other languages
and completion UI integration are pending. No project-wide autocomplete
claim follows from the candidate API alone.

## JavaScript definition extraction (item 251)

The first-open walker now ships a separate pinned tree-sitter JavaScript
parser (tree-sitter 0.22.6, JS grammar 0.23.1). It emits named functions,
generator declarations, classes and identifier-named methods, with 1-based
line and 0-based UTF-8 byte column. Comments, calls, strings and computed
method names are not definitions. File counts still count files, not records.

This does not reuse Lexilla paint state or user-installed grammars. TypeScript,
Vue, PHP and Liquid extraction remain pending. Local variables, imports,
references, type resolution, incremental edits and completion UI integration
are not included. Error-tolerant trees may yield partial definitions from
broken source; these candidates are hints, not safe rename targets. Each file
currently creates a fresh parser, so performance still needs measurement.

The extraction runtime uses the same native tree-sitter 0.22.6 as core.
Grammar 0.23.1 normally depends only on tree-sitter-language ^0.1, not a
second native runtime. A documented LanguageFn raw-pointer bridge constructs
0.22.6's Language from the generated static grammar pointer. Grammar ABI 14
is in runtime's accepted 13..14 range, checked by a test and set_language.
Runtime cc ^1.0.95 and grammar cc ^1.1 both accept the workspace's cc 1.2.
The grammar's newer runtime dependency is dev-only and is not used here.
