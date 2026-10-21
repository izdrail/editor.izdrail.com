# Hover help for Alpine and Livewire attributes (item 334)

Blade hover already explained directives (`@foreach`) and `<x-...>` props. Hovering `x-data`, `x-on:click.prevent`, `wire:model.live` and similar showed nothing. `hover_help` now has a table of 18 Alpine and 16 Livewire 3 attributes with syntax and a one-line description. The hovered token is reduced to its base name: arguments (`x-on:click` to `x-on`) and modifiers (`.live`, `.prevent`) are dropped. The attribute must follow whitespace, so `foo-x-data` is ignored; unknown attributes get no hover. No project access is needed.

Judged from the Alpine 3 and Livewire 3 docs as I know them, not a fetched copy. `wire:model` is described as deferred by default with `.live` for realtime, which is the Livewire 3 behaviour.

Capture: hover `x-data`, `x-on:click.prevent` and `wire:model.live` in a `.blade.php` file.
