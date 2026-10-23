# Alpine/Livewire magic property completion (item 367)

Typing `$` (or a partial like `$ne`) inside an Alpine expression
attribute value (`x-show`, `x-text`, `@click`, `x-on:*`, `x-bind:*`,
`:class`, `x-if`, `x-init`, `x-effect`, `x-model`, `x-for`, ...) or a
`wire:` attribute value offers the magic properties as Project items:

- Alpine: `$el`, `$refs`, `$store`, `$watch`, `$dispatch`, `$nextTick`,
  `$root`, `$id`, `$data`.
- Livewire: `$wire`, `$js`, `$entangle` (common inside `@click` and
  `wire:` values).

The context needs the caret inside an open quoted value of one of those
attributes, ending with `$` plus an optional partial identifier. The
partial must be empty or the start of a known magic name, so an
unrelated `$foo` does not raise an irrelevant list (it falls back to
the attribute's ordinary context, if any).
`foo$`, `$$` and `$this->` are not magic starts; plain attributes
(`class`, `href`) and `x-data` itself are not magic contexts. The more
specific `$store.` / `$refs.` completions (items 351/352) and helper
calls (`$dispatch('...`) keep their own contexts. In Blade files the
popup opens on the bare `$`; elsewhere it opens on the first letter.

Limits: inline `<script>` bodies are not a magic context; `$wire.`
member paths are a separate concern (hover covers the magics
themselves, item 350).

Tests: `detects_alpine_magic_context`, `offers_alpine_magics`.
