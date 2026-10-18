# Completion popup

The popup blends these sources and filters them with the existing `nucleo`
fuzzy matcher:

| Source | Priority | Notes |
| --- | --- | --- |
| LSP | first | Items keep their own edits, `sortText`, `filterText` and lazy resolve |
| Snippets | second | Imported from `.vsix` extensions (see [vsix-import.md](vsix-import.md)), picked by the file's language id, re-indented to the current line, inserted with tab stops |
| Language keywords | between snippets and words | PHP only for now: the keyword, builtin function, builtin type and constant lists the PHP lexer highlights |
| Project strings | between keywords and words | Inside `route('...')`, `config('...')`, `env('...')`, `view('...')`, `@section` / `@yield` / `@push` / `@stack('...')`, `__('...')` / `trans(...)` / `@lang(...)` / `Lang::get('...')` and `wire:model="..."` / `wire:click="..."` (and the other `wire:` event attributes): the app's route names, config keys, env keys, view names, section/stack names, translation keys and the paired Livewire component's public properties and methods; static scans cached 30 s per workspace. In a `.blade.php` file, typing `$` also offers the variables the view defines above the cursor (`$name = ...` assignments, `@foreach` / `@forelse` loop variables, `@props` entries and `@inject` names), replacing the whole-buffer variable word scan there. Typing `<x-` in a Blade view offers the app's component tags (`resources/views/components`, dots for nested components, an `index.blade.php` names its directory) |
| Buffer words | third | Identifiers from the current file, most frequent first; offered after 2 typed characters; the word being typed is excluded |

Ranking is the fuzzy score plus a bonus for a prefix match and a small source
bonus (LSP 24, snippet 12, word 0), then `sortText`, then label length. A word
that the server or a snippet already offers is not repeated. Local sources work
without a language server, but only for files with a path (not scratch buffers).

In PHP files, `$variable` names from the file are offered too; the label keeps the `$`, the inserted text does not (the word being replaced never includes it).

Not included: ghost text / AI completion, project-wide word indexing, ctags.
