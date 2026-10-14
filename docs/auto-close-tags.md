# Auto-close tags

In HTML, Twig, Blade, Vue and similar templates, typing `>` at the end of an opening tag inserts the closing tag and keeps the caret between them: `<div class="card">` becomes `<div class="card">|</div>`.

Applies to `.html`, `.htm`, `.xhtml`, `.twig` (including `.html.twig`), `.blade.php`, `.phtml`, `.vue` and `.svelte`. Plain `.php` files are not touched. Blade components (`<x-alert>`, `<x-slot:title>`), Livewire (`<livewire:search-box>`) and `<twig:...>` components close with their full name. Multi-line opening tags work (the last 4 KB before the caret are read).

Left alone: void elements (`br`, `img`, `input`, ...), self-closing tags (`<x-alert />`), `<?php`, `<!-- -->`, `<!DOCTYPE>`, arrows and comparisons (`$a->b>`, `=>`), a `>` typed inside an unclosed attribute quote or `{{ }}` / `{% %}`, and a tag that is already closed right after the caret.

Turn it off with `auto-close-tags = false` under `[editor]` in settings. Undo removes the typed `>` and the closing tag together with the typing group.

## Blade directives

In `.blade.php` files, pressing Enter at the end of a whole-line block directive adds the matching end directive below the new line and leaves the caret on the new line:

```
@foreach ($users as $user)
    |
@endforeach
```

Covered: `@if`, `@unless`, `@foreach`, `@forelse`, `@for`, `@while`, `@isset`, `@empty(...)`, `@auth`, `@guest`, `@can`, `@cannot`, `@canany`, `@env`, `@production`, `@error`, `@hasSection`, `@sectionMissing` (end with `@endif`), `@once`, `@verbatim`, `@push`, `@prepend`, `@pushOnce`, `@prependOnce`, `@pushIf`, `@fragment`, `@session`, `@switch`, `@component`, `@slot(...)`, `@section('x')` (one argument), a bare `@php` and `@teleport`.

Skipped: inline forms (`@if($a) x @endif`, `@php($x)`, `@section('a', 'b')`), `@else`, `@elseif`, `@include` and other non-block directives, multi-line arguments, text after the caret on the line, and blocks that already have their end directive further down (nested blocks are counted; the next 200 KB are scanned). The same `auto-close-tags` setting controls this.
