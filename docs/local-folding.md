# Local folding groundwork

Indentation-based code folding: range detection, a tested buffer/visible-line
projection, hidden-line support in the editor layer, gutter markers and commands.

`Syntax::parse` no longer builds tree-sitter trees after the Lexilla change.
The tree-sitter dependency and old code remain but there is no live syntax tree
to query. Indentation is used instead, without loading grammar shared libraries,
launching a language server, or executing a plugin.

The syntax worker computes indentation ranges and publishes them only if its
revision still matches the buffer. Plain-text and scratch buffers work too.
Edits discard stale ranges while the worker is running. Block endings are
exclusive: a dedented closing brace remains visible; EOF can be an ending.
Blank lines do not open or close blocks, tabs use four-column stops, and trailing
blank lines are excluded. This is a heuristic: unindented code and flat multiline
comments do not get ranges, while indented continuations can get ranges.

`FoldProjection` merges nested active folds into visible outer folds and maps
buffer lines to visible lines and back. It is a line-level model, not a wrapped
visual-line implementation. The caller must retain nested fold state separately.

## How folding works now

**Hidden lines in the editor layer.** `vendor/floem` is the pinned Floem revision plus
`Lines::set_hidden_lines(HiddenLines)`. Hidden buffer lines have no visual line: last
vline/rvline, buffer-line/offset to vline/rvline, vline to offset and forward/backward
iteration all skip them. Paint, hit testing, selection, caret, vertical movement, viewport
height and scroll-to-caret already derive from those conversions, so they follow without
separate code. An offset inside a hidden line is placed on the fold header (first visual
line, caret at its end). Buffer text, offsets, selections, copy and save are untouched.
While anything is hidden the lines are not "linear" and several conversions scan the
buffer lines (O(lines) per call, only while a fold exists) instead of using the cached
last visual line. Moving down from the last shown line stays on it when the end of the
file is folded.

**Fold state** is `lapce_core::folding::FoldState` (pure data, unit tested): the foldable
blocks, the folded ones (nested folds are kept when an outer block toggles), fold/unfold/
toggle/fold all/unfold all/reveal, edit handling and the hidden line ranges. It is stored
per document in `Doc::fold_state` and applied to every normal editor of the document when
screen lines are computed (`Doc::compute_screen_lines`). Diff editors never hide lines.

**Edits.** Blocks before an edit stay, blocks after it move by the change in line count,
blocks the edit touches (header, body) unfold. Foldable blocks are recomputed by the syntax
worker; folds whose header still starts a block are kept.

**Reveal.** When the caret ends up inside folded text (search, go to definition, go to
line, left/right movement) the blocks hiding it unfold. Fold commands move a caret that
would be hidden to the end of the header first.

**Commands** `fold`, `unfold`, `toggle_fold`, `fold_all`, `unfold_all` (editor focus):
Linux/Windows `ctrl+shift+[`, `ctrl+shift+]`, `ctrl+k ctrl+l`, `ctrl+k ctrl+0`,
`ctrl+k ctrl+j`; macOS `meta+alt+[`, `meta+alt+]`, `meta+k meta+l`, `meta+k meta+0`,
`meta+k meta+j`. `fold` folds the innermost unfolded block around the caret line, so
repeating it folds outwards.

## Not verified (needs a display)

Gutter marker placement and icons, wrapped lines next to folds, split editors showing the
same document, scroll position and scrollbar extent after folding or unfolding, scroll
to the caret after a reveal, mouse clicks below a folded block, selection painting across
a fold, and the keyboard shortcuts on each platform.
