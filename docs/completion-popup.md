# Completion popup

The popup blends three sources and filters them with the existing `nucleo`
fuzzy matcher:

| Source | Priority | Notes |
| --- | --- | --- |
| LSP | first | Items keep their own edits, `sortText`, `filterText` and lazy resolve |
| Snippets | second | Imported from `.vsix` extensions (see [vsix-import.md](vsix-import.md)), picked by the file's language id, re-indented to the current line, inserted with tab stops |
| Buffer words | third | Identifiers from the current file, most frequent first; offered after 2 typed characters; the word being typed is excluded |

Ranking is the fuzzy score plus a bonus for a prefix match and a small source
bonus (LSP 24, snippet 12, word 0), then `sortText`, then label length. A word
that the server or a snippet already offers is not repeated. Local sources work
without a language server, but only for files with a path (not scratch buffers).

Not included: ghost text / AI completion, project-wide word indexing, ctags.
