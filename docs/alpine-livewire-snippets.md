# Alpine and extra Livewire attribute snippets (item 331)

Blade files had Livewire snippets for click, model, loading, poll and navigate, but nothing for Alpine, which most Livewire 3 views also use (Livewire 3 ships Alpine). Added to `defaults/snippets/blade.json`:

- Alpine: `x-data`, `x-init`, `x-show`, `x-on`, `x-bind`, `x-model`, `x-text`, `x-html`, `x-ref`, `x-cloak`, `x-transition`, `x-for`, `x-if`, `x-effect`, `x-teleport`, `x-ignore`, `x-id`, `x-modelable`.
- Livewire 3: `wire-init`, `wire-ignore`, `wire-stream`, `wire-target`, `wire-transition`.

`x-on` expands to `x-on:click`, not `@click`: in Blade files `@` starts a directive, so the shorthand clashes. Judged against the Alpine and Livewire 3 docs from memory of their directive lists, not a fetched copy. Newer Livewire directives (for example `wire:show`) were left out because I could not confirm which Livewire 3 version added them.

Capture: in a `.blade.php` file type `x-d` and `wire-t` and check the completion list and the inserted text.
