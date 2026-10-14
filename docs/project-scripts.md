# Project scripts in Run and Debug

The Run and Debug palette also lists scripts found in the workspace root:

| Found | Entries |
| --- | --- |
| `package.json` | `npm: <script>` (or `pnpm:`, `yarn:`, `bun:` when that lockfile exists) running `<runner> run <script>` |
| `composer.json` | `composer: <script>` running `composer run-script <script>` |
| `artisan` | `artisan: <command>` for about 30 common framework commands (serve, test, tinker, migrate*, db:seed, route:list, queue:work, cache/config/view clears, `make:*`), plus the project's own commands from `$signature` in `app/Console/Commands/*.php` and `Artisan::command()` in `routes/console.php`; each runs `php artisan <command>` in the terminal (`make:*` prompt for a name there) |
| `bin/console` | `symfony: console` |

They run in the terminal like any run configuration, with the workspace as the
working directory. A `.lapce/run.toml` entry with the same name wins, and the
run.toml editor is no longer opened automatically when scripts were found.
Lifecycle hooks (`pre<name>`/`post<name>` when `<name>` exists, `pre-*`/`post-*`
composer events) are not listed. Files are only read (1 MB limit); nothing is
executed to discover scripts. Only the workspace root is scanned, so monorepo
sub-packages need run.toml entries.

Artisan discovery is static: `php artisan list` is never run, so commands provided by
packages are not listed unless they are in the built-in set. Add them to `.lapce/run.toml`.

Composer projects also list `composer: install`, `update`, `dump-autoload`, `outdated` and `audit`
(a script of the same name in `composer.json` replaces the built-in). When `bin/console` exists,
Symfony projects list `symfony: <command>` for cache:clear, debug:router/container/config,
lint:twig/yaml, Doctrine migrations and schema validate, messenger:consume and make:controller/entity/migration.
