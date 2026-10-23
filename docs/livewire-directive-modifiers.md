# Livewire directive modifiers (item 362)

Modifier completion after the dot now covers the non-event `wire:`
directives too, not only `wire:model` and the action events:

- `wire:loading.` offers `class`, `class.remove`, `attr`, `remove`,
  `delay`, `delay.shortest`, `delay.longer`, `delay.longest`
- `wire:dirty.` / `wire:offline.` offer `class`, `class.remove`,
  `attr`, `remove`
- `wire:navigate.` offers `hover`
- `wire:ignore.` offers `self`

These directives live in a new `MODIFIER_DIRECTIVES` list rather than
`EVENTS`, because their value is not a component method: adding them to
`EVENTS` would have made `wire:loading="..."` complete action methods.
Attribute names (`loading`, `dirty`, `navigate`, ...) were already
offered after `wire:` from the hover-help table.

Tests: `offers_livewire_directive_modifiers`.
