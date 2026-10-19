# Project index (lapce-index)

The editor keeps one embedded [tantivy](https://github.com/quickwit-oss/tantivy)
index per workspace. It is the single structure behind file search,
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
- `add_entry` queues a record; `remove_path` queues deletion of every record
  for one path (used on save: delete, then re-add that file's records);
  `commit` flushes and explicitly reloads the reader, so searches made
  immediately afterward see the change without waiting for a watcher.
- `ProjectIndex::in_memory()` is the volatile variant for tests.

## Build plan status

1. (this patch) Crate skeleton: schema, add/remove/commit/search, unit tests.
2. First-open indexer: walk the workspace (respecting .gitignore), index every
   file on a background thread with status-bar progress.
3. tree-sitter symbol extraction per saved file (delete-by-path, re-add).
4. Completion cascade: local index -> LSP -> llm.izdrail.com.
5. Per-framework LSP presets (PHP/Magento/WordPress/Symfony, TS/React/Angular,
   Vue via Volar, Liquid via Shopify theme-check).
6. MCP presets and ACP docs for Claude/Codex/opencode.
