# Project scripts in Run and Debug

The Run and Debug palette also lists scripts found in the workspace root:

| Found | Entries |
| --- | --- |
| `package.json` | `npm: <script>` (or `pnpm:`, `yarn:`, `bun:` when that lockfile exists) running `<runner> run <script>` |
| `composer.json` | `composer: <script>` running `composer run-script <script>` |
| `artisan` | `artisan: serve`, `artisan: test`, `artisan: migrate` |
| `bin/console` | `symfony: console` |

They run in the terminal like any run configuration, with the workspace as the
working directory. A `.lapce/run.toml` entry with the same name wins, and the
run.toml editor is no longer opened automatically when scripts were found.
Lifecycle hooks (`pre<name>`/`post<name>` when `<name>` exists, `pre-*`/`post-*`
composer events) are not listed. Files are only read (1 MB limit); nothing is
executed to discover scripts. Only the workspace root is scanned, so monorepo
sub-packages need run.toml entries.
