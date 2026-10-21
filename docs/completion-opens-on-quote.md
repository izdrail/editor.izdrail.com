# Completion opens on the bare quote of a known argument (item 346)

Typing `route('`, `@include('`, `@livewire('`, `config('`, `__(` or
`wire:model="` used to leave the completion popup closed until a letter
was typed: `-`, `'` and `"` are not trigger characters, so the empty-input
gate cancelled. The gate now also opens when
`project_completion::context()` recognises the position (a quoted argument
the project has items for, or a Livewire attribute value), and all
project-sourced items carry `show_on_empty`, so the list is populated
immediately. Unknown callers (`console.log('`), closed strings and later
arguments still open nothing.

Tests: `context_detects_the_bare_quote` pins the contexts that open.
