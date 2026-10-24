# Built-in artisan commands in Artisan::call/queue (item 409)

`Artisan::call('mi` and `Artisan::queue('qu` now also offer the
built-in Laravel command names (`migrate`, `queue:work`, ...),
alongside the project's custom commands from item 408.

The list is `BUILTIN_COMMANDS` in `artisan_index.rs`: 113 names
harvested from the laravel/framework 12.x tree (12.x @ 3f732656) -
the first word of every `$signature` and `$name` in
`src/Illuminate/**/Console/**`, filtered to command-name shape.
Non-command `$name` values (`default`, `local`), signature
arguments and hidden commands (`invoke-serialized-closure`) are
excluded; `env` stays because
`Illuminate\Foundation\Console\EnvironmentCommand` is a real
command, and hyphenated names like `clear-compiled` are included.
The list is static and best-effort: it is not re-read from the
vendor tree at runtime, so brand-new framework commands appear only
when the list is refreshed.

Tests: the updated `offers_artisan_commands` pins built-in presence
(`migrate`, `queue:work`) next to the custom commands and keeps the
signature-argument and commented-command exclusions.
