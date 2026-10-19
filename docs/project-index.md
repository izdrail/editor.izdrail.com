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
