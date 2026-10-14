# CSS / Tailwind Classes in Use

Command palette: **CSS / Tailwind Classes in Use** (`palette.css_classes`).

Lists every class name written in a `class="..."` or `className="..."` attribute under `resources/` (`.php` and `.blade.php`, `.html`, `.vue`, `.svelte`, `.jsx`, `.tsx`, `.astro`, `.twig`), most used first, e.g. `p-4  x37  resources/views/home.blade.php:12`. Picking one jumps to its first use (files are visited in path order). Type in the filter to narrow the list, for example `p-4`, `md:` or `hover:`.

Good for finding near-duplicates (`p-4` next to `p-[16px]`), checking how widely a utility is used before changing it, and finding where a custom class lives.

The list is built only from your files. It does not know Tailwind's class list, does not read `tailwind.config.js`, and works the same for Bootstrap or your own CSS. It does not suggest or insert classes.

Skipped: classes built by concatenation or template syntax (`{{ }}`, `<?php`, `$var`, `@if` inside the attribute), `:class` bindings, `class` in JavaScript objects, and tokens longer than 80 characters or with unusual characters. Counts are per occurrence. Static text scan only; no PHP is run. Limits: depth 8, 3000 files, 1 MB per file, 2000 bytes per attribute, the 1000 most used classes are listed, symlinks not followed.

Status: unit tests cover extraction (quotes, multi-line, arbitrary values, `subclass=` ignored), counting and ordering, caps, symlinks and random-input fuzz. The palette GUI is not yet verified.
