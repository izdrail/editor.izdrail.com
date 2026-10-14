# Go to Artisan Command

Command palette: **Go to Artisan Command** (`palette.artisan_commands`).

Lists your project's own Artisan commands, e.g. `mail:send  Send it  app/Console/Commands/Send.php`, and jumps to where each is defined. It does not run anything; to run a command use your terminal or a run config.

Sources, by text scan only (no PHP, no `artisan` is executed):
- `app/Console/Commands/**.php`: `$signature = '...'` (name is its first word), `#[AsCommand(name: '...')]` or `$name = '...'`, plus `$description` when it is on one line.
- `routes/console.php`: `Artisan::command('name {arg}', ...)`.

Not covered: framework and vendor commands, commands registered from other folders, signatures built by concatenation or constants. Limits: depth 6, 1000 files, 256 KB read per file, 2000 entries, symlinks not followed.

Status: unit tests cover every syntax form, discovery, caps, symlinks and random-input fuzz. The palette GUI is not yet verified.
