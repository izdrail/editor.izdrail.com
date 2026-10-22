# Alpine $refs completion (item 352)

Typing `$refs.` in a Blade attribute expression or inline script offers
the `x-ref="name"` values of the current file (from the saved file, like
the Livewire property list), labelled "Alpine x-ref". Only the first
segment after `$refs.` is completed; a path such as `$refs.a.b` stays out
of context. Bounded: 1 MB file, 100 refs.

Tests: `detects_alpine_ref_context`, `offers_alpine_refs`.
