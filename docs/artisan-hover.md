# Artisan command hover

Hovering a console command name shows the command's signature, description
and source line - the same static scan as **Go to Artisan Command**, so the
two always agree:

    Artisan::call('mail:send', ['user' => 1])
        -> **`mail:send`** - Send the mail
           `mail:send {user} {--force}`
           app/Console/Commands/SendMail.php:3

Recognised call sites (PHP files only): `Artisan::call`, `Artisan::queue`,
`Artisan::callSilent`, `Artisan::command` (the definition itself),
`Schedule::command` / `$schedule->command(...)`, `$this->call(...)` and
`$this->artisan(...)`. The name is the first word of the quoted string, so
`Artisan::call('mail:send --force')` still resolves.

Sources: `app/Console/Commands/**.php` (`$signature`, `$description`,
`#[AsCommand]`, `$name`) and `Artisan::command(...)` closures in
`routes/console.php`. Framework and vendor commands are not indexed; an
unknown name falls through to the language server.

Nothing is executed. Bounded: depth 6, 300 files, 256 KB per file, symlinks
skipped.
