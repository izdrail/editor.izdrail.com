# Import PHP Class

Command palette: **Import PHP Class** (`import_php_class`, no default key). Put the cursor on a class name in a `.php` file and run it. A `use Vendor\Package\Class;` line is added in sorted position among the existing imports.

- One match: the import is inserted straight away (one undo step).
- Several matches (`User` in `App\Models` and `App\Admin`): a palette lists the full names, project code first. Type to filter, Enter inserts.
- No imports yet: the line goes after `namespace ...;` (or after `<?php`) with a blank line around it.
- Already imported, or the class is in the file's own namespace: a toast says so and nothing is changed.

## Where classes are found

Static only, nothing is executed:

1. `autoload` and `autoload-dev` PSR-4 entries of `composer.json` (default `App\` => `src/`).
2. Installed packages from `vendor/composer/installed.json` (their PSR-4 `autoload` entries). Package paths that resolve outside `vendor/` are ignored.

The directories are walked for a file named `<Class>.php`. Limits: 40,000 directory entries, depth 14, 1.5 s, 50 results, `installed.json` up to 16 MB. Symlinks, `node_modules` and dot directories are skipped.

Not covered: PHP global classes (`Exception`, `DateTime`), classmap-only and PSR-0 packages, `use function`/`use const`, and a file that declares several classes. CRLF files keep CRLF.
