# Blade snippets survive label dedupe (item 357)

In `.blade.php` files a snippet now ties with LSP (tier 0) when several
sources offer the same label, so a bare LSP or keyword `foreach` can no
longer hide the directive snippet that expands the whole
`@foreach ... @endforeach` block. Both items appear (LSP first, snippet
right after); buffer words with the same label are still dropped, and
dedupe everywhere else is unchanged.

Root cause of the missing `@foreach` snippet in the 0355 GUI check: the
completion list keeps only the highest-priority source per label, and an
LSP keyword item outranks a snippet. The snippet was present but deduped
away before rendering.

Tests: `blade_keeps_directive_snippet_beside_lsp_keyword`.
