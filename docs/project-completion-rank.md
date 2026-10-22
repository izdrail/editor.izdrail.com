# Project suggestions rank above snippets, keywords and words (item 358)

Source tiers are now LSP, Project, Snippet, Keyword, Word. Project items
exist only when the line is a recognised project context (`route('...`,
`wire:model="...`, `$store.`, `<x-...` and friends), so promoting them
cannot disturb a context they are absent from; inside their context they
are the relevant answer (`email` above `empty`/`wire:model` in
`wire:model="em`, `loading` above `wire-loading`/`slot` in `<div wire:l`).
Blade view variables are Project-sourced and rise with them.

Label dedupe is unchanged outside Blade. In Blade, snippets now keep
their label regardless of source (they bypass cross-source dedupe and
only dedupe among themselves), which replaces the 0357 tier tie: a
project property and a same-named snippet (`isset` in `wire:model="..."`)
both stay available.

Tests: `items_sort_by_source_then_score`,
`blade_keeps_directive_snippet_beside_lsp_keyword`.
