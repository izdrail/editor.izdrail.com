# Prettier formatting (format-on-save and Format Document)

When a project opts in to Prettier, the editor formats through it instead of
asking a language server or plugin. A file qualifies when:

- its extension is one of js, jsx, mjs, cjs, ts, tsx, json, css, scss, less, html, vue, md, yaml, yml;
- a Prettier config exists in the file's folder or a parent (`.prettierrc*`,
  `prettier.config.*`, or a `"prettier"` key in `package.json`); and
- a `prettier` binary is found: the nearest `node_modules/.bin/prettier`, else `prettier` on `PATH`.

The buffer text (including unsaved edits) is piped to `prettier --stdin-filepath <file>`
in the project root, with a 0.9 second limit. If it fails, times out or is missing, no
edits are made and the save goes ahead without formatting; there is no fallback to a
second formatter. Output over 4 MB is ignored. The result replaces the whole document as one undoable edit.

Format-on-save itself is still controlled by `Editor > Format On Save` (off by default);
"Format Document" uses the same path. ## PHP: Laravel Pint and php-cs-fixer

For `.php` files in a Composer project (nearest folder with `composer.json`):

- `vendor/bin/pint` present: Pint formats the file, with `pint.json` from the project root if it exists
  (otherwise Pint's defaults).
- else `vendor/bin/php-cs-fixer` plus `.php-cs-fixer.php` or `.php-cs-fixer.dist.php`: php-cs-fixer
  runs with that config (`--using-cache=no`). A fixer binary without a config is not treated as opt-in.
- neither: PHP keeps using language servers as before.

Both tools only format files, so the buffer (including unsaved edits) is written to a temporary
`.php` copy, formatted there, and read back; the real file is never touched and the copy is deleted.
The limit is 4 seconds (they boot the Composer autoloader). Failure, timeout or output over 4 MB means
no edits and the save goes ahead. A Pint or fixer config that depends on the file's own path
(for example `notPath` rules) is evaluated against the temporary path, so such rules are not applied.
PHP is run only by these project-local binaries, only on format, and only if the project has them.

Other languages keep using language servers and plugins.
