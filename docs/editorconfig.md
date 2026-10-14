# .editorconfig support

The editor reads `.editorconfig` for local files. Before this it did not read it at all. Supported properties:

| Property | Effect |
| --- | --- |
| `indent_style` (`tab`/`space`) | Indent unit for Tab, auto-indent and Indent/Outdent. Beats the width the editor guessed from the file. |
| `indent_size` (number), `tab_width` (number) | Spaces per indent level (1 to 8). `indent_size` alone only changes files that use spaces. `indent_size = tab` is treated as unset. |
| `end_of_line` (`lf`/`crlf`) | On save the buffer is converted to that line ending (`cr` is ignored). |
| `trim_trailing_whitespace` (`true`) | On save, spaces and tabs at the end of lines are removed. |
| `insert_final_newline` (`true`/`false`) | On save, a final line break is added, or one is removed. Empty files stay empty. |

Lookup follows the EditorConfig spec: from the file's folder upwards until a file with `root = true`
(at most 24 folders; each `.editorconfig` up to 64 KB), nearer files and later sections win, `unset` clears a
value. Globs: `*`, `**`, `?`, `[abc]`, `[!abc]`, `{a,b}`, `{1..3}`; a glob without `/` matches the file name at any
depth, one with `/` is relative to that `.editorconfig`'s folder. Property names and values are case-insensitive.

Notes:

- Indentation is applied when a file is opened. Edit `.editorconfig`, then reopen the file to pick it up. The save
  rules are read on every save.
- The save rules run on a normal save (not a "save without formatting") and only for local workspaces; remote
  workspaces are not read. Buffers over 5 MB skip trimming and final newline handling.
- Not used: `charset`, `max_line_length` and the display `tab_width` (the global Tab Width setting still draws tabs).
- Unrelated to Format on Save, which stays off by default.
