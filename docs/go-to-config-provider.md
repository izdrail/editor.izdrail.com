# Go to Config / Service Provider

Command palette: **Go to Config / Service Provider** (`palette.config_providers`).

- `[Config] services.mail  config/services/mail.php`: each file under `config/` named the way you call it, `config('services.mail.key')`. Opens the file at line 1. To jump to an individual key use **Go to Env / Config Key**.
- `[Provider] AppServiceProvider  app/Providers/AppServiceProvider.php`: each class under `app/Providers`, plus `[Provider] AppServiceProvider::register()` and `::boot()` entries that open at those methods.

Static text scan only; no PHP is run. Providers that are registered but live outside `app/Providers`, and `bootstrap/providers.php` ordering, are not covered. Limits: config depth 4, providers depth 6, 1000 files, 128 KB read per provider, 3000 entries, symlinks not followed.

Status: unit tests cover name mapping, provider parsing, discovery, caps, symlinks and random-input fuzz. The palette GUI is not yet verified.
