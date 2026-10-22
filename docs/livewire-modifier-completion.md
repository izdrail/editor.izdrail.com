# `wire:` modifier completion (item 347)

Typing a dot after a Livewire event attribute name offers the Livewire v3
modifiers for that event, before any `=`:

- `wire:model.` -> live, blur, change, lazy, fill, debounce.150ms,
  throttle.500ms
- `wire:keydown.` / `wire:keyup.` -> enter, escape, space, tab, backspace,
  the arrow keys, shift, ctrl, alt, meta
- `wire:click.` / `wire:submit.` / `wire:dblclick.` / `wire:mouseenter.` /
  `wire:mouseleave.` -> prevent, stop

Events without modifiers (poll, init, intersect, ...) offer nothing. Once
`="` starts the value, the existing property/action completion takes over
unchanged. The list opens on the dot and filters per keystroke; accepting
replaces only the partial modifier.

Tests: `offers_livewire_modifiers`.
