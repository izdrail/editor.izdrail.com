# Artisan command completion (item 408)

`Artisan::call('re` and `Artisan::queue('re` offer the project's
custom artisan command names (Laravel 12.x Console Kernel contract:
`call($command, ...)`, `queue($command, ...)`).

Names come from the go-to palette's index sources, by text scan with
nothing run: the `$signature` / `#[AsCommand(name: ...)]` / `$name`
first word of each class in `app/Console/Commands/**.php`, and
`Artisan::command('name ...')` closures in `routes/console.php`. The
same depth, file-count, byte and entry bounds as the palette index
apply, and signature arguments (`reports:send {user}` offers
`reports:send`) and commented-out commands invent nothing. Built-in
Laravel commands (`migrate`, `queue:work`, ...) are also offered,
from a static list harvested from the laravel/framework 12.x tree
(item 409, `BUILTIN_COMMANDS`).

Only a scalar name completes; the array argument form stays silent.
The receiver must be exactly `Artisan::` - `MyArtisan::call(...)`,
`$artisan->call(...)` and the `$schedule->command('...')` object
receiver stay silent - and a call written inside a comment or a
string literal is not a call.

Tests: `detects_artisan_command_context`, `offers_artisan_commands`.
