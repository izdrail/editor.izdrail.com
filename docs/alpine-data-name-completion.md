# Alpine.data component name completion (item 369)

Inside `x-data="..."` the popup offers the component names registered
with `Alpine.data('name', ...)` in the project's `resources/js`,
`resources/ts`, `public/js` and inline Blade `<script>` blocks, as
Project items. The whole quoted value must be a partial name (letters,
digits, `_`, `-`), so `x-data="dropdown({ cou` falls out of context
once the call begins and `x-data="{ open: fa` (an inline object) is not
a name context. A bare `x-data="` lists everything; in Blade files the
popup opens on the bare quote.

Textual scan, nothing executed; same bounds as the store scan (item
351): depth 8, 400 files, 1 MB per file, 200 names, symlinks skipped.
A longer attribute containing `x-data=` (`data-x-data=`) and values
with spaces around `=` are not recognised.

Related: the x-data property completion (item 365) reads inline object
literals and, for a component referenced by name, resolves the keys of
its registered factory (item 370).

Tests: `detects_alpine_data_context`, `offers_alpine_data_names`.
