# Alpine and extra Livewire attribute snippets (item 331)

Blade files had Livewire snippets for click, model, loading, poll and navigate, but nothing for Alpine, which most Livewire 3 views also use (Livewire 3 ships Alpine). Added to `defaults/snippets/blade.json`:

- Alpine: `x-data`, `x-init`, `x-show`, `x-on`, `x-bind`, `x-model`, `x-text`, `x-html`, `x-ref`, `x-cloak`, `x-transition`, `x-for`, `x-if`, `x-effect`, `x-teleport`, `x-ignore`, `x-id`, `x-modelable`.
- Livewire 3: `wire-init`, `wire-ignore`, `wire-stream`, `wire-target`, `wire-transition`.

`x-on` expands to `x-on:click`, not `@click`: in Blade files `@` starts a directive, so the shorthand clashes. Judged against the Alpine and Livewire 3 docs from memory of their directive lists, not a fetched copy. Newer Livewire directives (for example `wire:show`) were left out because I could not confirm which Livewire 3 version added them.

Capture: in a `.blade.php` file type `x-d` and `wire-t` and check the completion list and the inserted text.

## Hyphenated prefixes (item 332)

The completion word stopped at the hyphen, so choosing `wire-target` after typing `wire-t` left the typed `wire-` and inserted the body after it (`wire-wire:target=...`), and `x-d` matched only `d` (ranking `disabled` above `x-data`). In `.blade.php` files `update_completion` now moves the word start back over hyphenated segments (`hyphen_word_len`, unit-tested), so the replace range and the fuzzy input are the whole `x-d` / `wire-t`. After a typed `-` the input is now `x-` rather than empty, so the popup no longer closes on the hyphen. Side effect: `<x-d` also replaces the `<`, as `<livewire` snippets already did.
