# PHP-file gate for whole-name completions (item 411)

The item 410 replacement fix extended the completion word start
over the whole typed name only in PHP files, but `items()` still
offered those completions anywhere - in a `.txt` buffer,
`Artisan::call('reports:r')` offered artisan commands whose accept
would graft the item onto the typed prefix, the same hybrid the
410 fix removed.

`items()` now returns nothing for the whole-name kinds (the
`dotted_name_kind` set: routes, config keys, views, sections,
stacks, translation keys, Gate abilities, Livewire names/events,
middleware aliases, storage disks, log channels, cache stores,
mailers, auth guards, database/Redis/queue/broadcast connections,
artisan commands) when the file is not a PHP file. The kind set is
now the single source of truth for both the replacement extension
and the offering gate, so the two cannot drift apart again. Blade
views end in `.blade.php` and stay covered; kinds outside the set
(table names, class symbols) use plain word boundaries on both
sides and are unaffected. An absent file argument is treated as
PHP - that is the unit-test calling convention.

Tests: `name_replacement_kinds_are_php_file_only`.
