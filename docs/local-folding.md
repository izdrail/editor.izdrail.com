# Local folding groundwork

This patch adds range detection and a tested buffer/visible-line projection. It
is not a complete folding UI. The gutter remains hidden and no shortcuts are
added, because the pinned Floem editor does not support hidden buffer lines.

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

## Why the old gutter cannot simply be enabled

The base already contains a hidden folding gutter, a no-op click handler,
commented-out LSP range requests and commented-out paint filtering. These
limitations also occur on the parent of the server-removal commit. They were not
introduced solely by removing the LSP runtime.

Only skipping painted rows leaves the editor's point-to-offset calculation,
selection painting, caret placement, vertical movement, viewport sizing and
scroll-to-caret using unfolded positions. It is not working folding.

## Remaining implementation

1. Add a hidden-line/projection API to the pinned Floem `Lines` layer, with
   consistent forward/backward wrapped-line iteration, buffer-to-visual mapping,
   visual-to-buffer mapping, final visual-line count and layout-cache invalidation.
2. Have all paint, hit testing, selection/caret, movement, scrolling and viewport
   code use that mapping. Preserve original buffer offsets, selection, copy and
   save data. Hidden targets from search/goto-definition must unfold their parents.
3. Connect document fold state to that API. Keep nested state when an outer fold
   toggles, move a cursor out of newly hidden content, and unfold/reset safely on
   edits. Consider keeping fold state per editor rather than per document.
4. Enable the gutter and implement fold/unfold current block and fold/unfold all
   commands, then give them collision-checked default shortcuts.
5. Visually test nested blocks, wrapping, split editors, diff views, search, EOF,
   scroll after large folds, caret/selection and mouse clicks below folded regions.

## Verified

- `cargo check --workspace -j 2`
- `cargo test -p lapce-core -j 2` (19 tests pass)

No GUI behavior is claimed or visually verified by this patch.
