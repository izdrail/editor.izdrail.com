# Emmet abbreviations (HTML and CSS)

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

Not yet: `lorem`, filters (`|c`), `@-` numbering modifiers and
auto-trigger on Tab. Output uses the buffer's indent unit and keeps the current
line's indentation. Invalid abbreviations do nothing.

## CSS, SCSS and Less files

In `.css`, `.scss` and `.less` files the same key expands property abbreviations
into one declaration:

| Abbreviation | Result |
| --- | --- |
| `m10` / `m10-20` / `m-10` | `margin: 10px;` / `10px 20px` / `-10px` |
| `m0-auto` | `margin: 0 auto;` |
| `w100p`, `fz1.5r`, `fz1e` | `%`, `rem`, `em` units (`p`, `r`, `e`); other letters pass through (`h50vh`) |
| `fw700`, `lh1.4`, `z10`, `op.5` | unitless properties keep plain numbers |
| `bgc#333!` | `background-color: #333 !important;` |
| `df`, `dg`, `pa`, `tac`, `jcsb`, `aic`, `fxdc` | whole-declaration shortcuts |
| `mt` (no value) | `margin-top: ;` with the caret in the value |

The property list is a deliberate common subset (margin, padding, size, border,
background, colour, font, position offsets, flex/grid basics), not the full Emmet
table. Unknown abbreviations do nothing.
