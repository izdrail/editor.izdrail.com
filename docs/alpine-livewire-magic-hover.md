# Alpine/Livewire magic property hover (item 350)

Hovering a magic property inside an Alpine or Livewire attribute
expression in a Blade file shows a short reference:

- Alpine: `$el`, `$refs`, `$store`, `$watch`, `$dispatch`, `$nextTick`,
  `$root`, `$id`, `$data`.
- Livewire: `$wire` (properties, methods, `$set`/`$toggle`/`$refresh`/
  `$parent`), `$js`, `$entangle`.

The hover only fires inside a quoted attribute value, so a plain Blade
`{{ $data }}` echo is not mislabelled as Alpine's `$data`; a component
prop declared in the file's own `@props` still wins over the magic name.

Tests: `magic_property_hover`.
