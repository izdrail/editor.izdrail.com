# Auto-close tags

In HTML, Twig, Blade, Vue and similar templates, typing `>` at the end of an opening tag inserts the closing tag and keeps the caret between them: `<div class="card">` becomes `<div class="card">|</div>`.

Applies to `.html`, `.htm`, `.xhtml`, `.twig` (including `.html.twig`), `.blade.php`, `.phtml`, `.vue` and `.svelte`. Plain `.php` files are not touched. Blade components (`<x-alert>`, `<x-slot:title>`), Livewire (`<livewire:search-box>`) and `<twig:...>` components close with their full name. Multi-line opening tags work (the last 4 KB before the caret are read).

Left alone: void elements (`br`, `img`, `input`, ...), self-closing tags (`<x-alert />`), `<?php`, `<!-- -->`, `<!DOCTYPE>`, arrows and comparisons (`$a->b>`, `=>`), a `>` typed inside an unclosed attribute quote or `{{ }}` / `{% %}`, and a tag that is already closed right after the caret.

Turn it off with `auto-close-tags = false` under `[editor]` in settings. Undo removes the typed `>` and the closing tag together with the typing group.
Blade `@if`/`@endif` style directives are not auto-closed; the snippet pack covers those.
