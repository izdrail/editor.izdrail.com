# wire:target completion

Inside a Livewire view, the value of `wire:target="..."` offers the
paired component's public actions first, then its public properties
(kebab-free, as declared) - the same static class scan as
`wire:model` / `wire:click` completion, so a `wire:loading` or
`wire:dirty` state can be scoped without leaving the editor.

## Context (item 379)

The caret must sit inside the open quoted value of a `wire:target`
attribute in a real tag. Comma-separated targets keep the context
(`wire:target="save, de`), matching Livewire's multi-action syntax;
the editor filters by the word after the last comma. `wire:target` was
already offered as an attribute name (item 356); this completes its
value.

Per Livewire's wire-loading docs, a target is an action name
(optionally with parameters) or a property name updated via
`wire:model`, so both lists complete. Parameter positions
(`wire:target="remove({{ ... }})"`) and closed values stay with the
generic suggestions; `data-wire:target` and `wire:loading="..."`
itself are not target contexts.

Tests: `offers_livewire_targets`, plus context pins in
`detects_contexts`-adjacent wire: tests.
