# PHP and Laravel lexers

Two Lexilla lexers were added next to the stock ones (`lexilla-rs/vendor/lexilla/lexers/LexPHP.cxx`
and `LexLaravel.cxx`, registered in `vendor/lexilla/src/Lexilla.cxx`, ids `SCLEX_PHP` 143 and
`SCLEX_LARAVEL` 144; lexers are looked up by name, `php` and `laravel`).

## Which lexer a `.php` file gets (`lapce-core/src/lexilla.rs`)

| File | Lexer |
| --- | --- |
| Starts with `<?php` (after optional BOM and whitespace), inside a Laravel project | `laravel` |
| Starts with `<?php`, any other project | `php` |
| Anything else (HTML first, `<?=` first, mixed templates) | `hypertext` as before |

Routing is by content on every highlight, so a file moves between lexers as it is edited.
A Laravel project is the nearest ancestor holding an `artisan` file, or a `composer.json`
that mentions `laravel/framework`. The result is cached per directory for 60 seconds.

## What they add

* `php`: nestable block comments, doc comments, `#[attributes]`, single/double quoted strings with
  `$var` interpolation, heredoc and nowdoc (including indented closers), `0x`/`0b`/`0o` numbers with `_`
  separators, `<?php`, `<?=` and `?>` transitions, keyword, builtin function, builtin type and
  constant lists.
* `laravel`: all of `php` plus facades (PascalCase before `::` or in the facade list), route methods and
  Eloquent methods after `::`, helpers when called, Artisan `a:b` commands from the list, `#[...]`
  attribute spans, middleware aliases inside single-quoted strings on a line that mentions `middleware`,
  and Blade directives (`@if`, `@endforeach`, ...) in inline HTML.

## Known limits

* `?>` inside a `//` or `#` comment does not end PHP mode (real PHP does).
* Nested `${...}` / `{$expr}` interpolation in double-quoted strings is only partly handled.
* The standalone lexers colour inline HTML as one style, which is why mixed templates stay on `hypertext`.
  Blade directives therefore only show in a `.php` file that starts with `<?php`.
* Folding: Lexilla fold levels are not used by the editor (it folds by indentation).
* Any `Name::` is coloured as a facade, so ordinary classes (`User::`) get the facade colour.
* Builtin function names have no colour of their own in `php` (the lexer tags them as identifiers).
* The `laravel` word lists are a starting set, not exhaustive.

Keywords, builtin functions, types and constants of the PHP lists also feed the completion popup
([completion-popup.md](completion-popup.md)), together with `$variable` names from the buffer.
