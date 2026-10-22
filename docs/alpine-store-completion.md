# Alpine store name completion (item 351)

Typing `$store.` in a Blade attribute expression or inline `<script>`
offers the store names registered with `Alpine.store('name', ...)` in the
project's `resources/js`, `resources/ts`, `public/js` and inline Blade
scripts, labelled "Alpine store". The check runs before the `wire:`
attribute scan, so `wire:click="$store.na" completes store names instead
of Livewire methods. A property path (`$store.nav.open`) stays out of
context: only the first segment after `$store.` is completed.

Textual scan, nothing executed. Bounded: depth 8, 400 files, 1 MB per
file, 200 store names, symlinks skipped; names containing a backslash are
ignored. The scan runs only in the `$store.` context, not for the other
project completions.

Tests: `detects_alpine_store_context`, `offers_alpine_stores`.
