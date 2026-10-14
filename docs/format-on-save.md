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
"Format Document" uses the same path. Files in other languages (including PHP) keep using
language servers and plugins. php-cs-fixer and Laravel Pint need file-based invocation
and project config discovery, so they are not wired yet.
