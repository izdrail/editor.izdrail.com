# Emmet abbreviations (HTML)

Type an abbreviation and press `Ctrl+Alt+M` (or run "Emmet: Expand Abbreviation"
from the command palette). The abbreviation is the run of text before the caret
back to the last space, quote or `<`. Expansion is applied as a snippet, so Tab
jumps to the first tabstop. Change the key in `keymaps.toml` (`emmet_expand`).

| Abbreviation | Result |
| --- | --- |
| `div.box>h1{Title}+p` | div with class, heading and paragraph |
| `ul>li.item$*3` | list with `item1`..`item3` |
| `(a+b)*2` | group repeated twice |
| `div>p^footer` | `^` climbs one level |
| `a[href=/x target=_blank]{go}` | attributes and text |
| `table>.row>.cell` | implicit tags: `tr`, `td` inside tables, `li` in lists |
| `a`, `img`, `input`, `link:css`, `script:src`, `form:post` | common defaults |
| `!` | HTML5 document skeleton |

Not yet: CSS abbreviations, `lorem`, filters (`|c`), `@-` numbering modifiers and
auto-trigger on Tab. Output uses the buffer's indent unit and keeps the current
line's indentation. Invalid abbreviations do nothing.
