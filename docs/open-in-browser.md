# Open in Browser

Command palette: "Open in Browser" opens the active `.html`, `.htm`, `.xhtml` or
`.svg` file as a `file://` URL in the system's default browser. Save first; the
browser reads the file from disk. PHP, Twig and other server-side files are
refused (they need a running server; use a Run and Debug script such as
`artisan: serve`). There is no live reload yet; refresh the browser tab yourself.
No keybinding is assigned; add one in `keymaps.toml` for `open_in_browser`.
