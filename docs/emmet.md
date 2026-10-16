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

Numbering modifiers work after `$`: `li.item$@-*3` counts down (3, 2, 1), `li.n$@5*3` starts at 5, `li.n$@-5*3` counts down ending at 5. Filter `|c` adds a closing comment after every element that has an id or class
(`div#page>ul.nav|c` gives `<!-- /.nav -->` and `<!-- /#page -->`); it is after-only, an
approximation of Emmet's comment filter. Other filters (`|e`, `|bem`) are not supported
and make the abbreviation invalid. Not yet: auto-trigger on Tab. Output uses the buffer's indent unit and keeps the current
line's indentation. Invalid abbreviations do nothing. So do runaway ones: more than 2000 elements after repeats are multiplied (`li*500>li*500`), more than 8 nested groups or 32 nested `>` levels.

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

## Colon snippets, lorem and CSS units

- Emmet's colon snippets from emmetio/emmet `html.json`: `input:text|email|password|search|url|tel|number|date|time|color|checkbox|radio|range|file` (type, name, id), `input:submit|button|reset|image|hidden`, `a:link|mail|tel|blank`, `meta:vp|utf|compat|edge|redirect|refresh|kw|desc`, `link:print|favicon|manifest|touch|rss|atom`, `script:module`, `form:get`, `btn:s|r|b`, `html:5` (same as `!`). Only the first empty attribute gets a tabstop; Emmet's mirrored name/id tabstop is not reproduced.
- An unknown colon name (`input:nope`), a name that starts with `@`, a digit or `-` is no longer expanded into a bogus `<input:nope>` tag; the abbreviation is left alone.
- `lorem`, `lorem12`, `lipsum`: placeholder text (30 words by default, 1 to 500). `p>lorem5` fills the paragraph, a lone `lorem5` is just text. The words come from a fixed list, so the text is similar to Emmet's, not identical.
- CSS: `m0-a` and `ml-a` mean `auto`; only real units are accepted after a number (`px em rem ex ch vw vh vmin vmax dvh svh lvh pt pc cm mm in fr s ms deg rad turn`, plus `p` for % and `e` for em), so `m0a` or `m10zz` no longer produce invalid CSS.
