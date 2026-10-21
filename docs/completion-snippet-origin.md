# Crash: snippet completion placed the cursor past the text (item 335)

Symptom: accepting a snippet completion with Enter (`x-data` after typing `x-d` in a Blade file) crashed the editor on the next paint, in lapce-xi-rope `is_char_boundary` (rope.rs:170) via `rvline_col_of_offset` and `paint_normal_selection`: the cursor held an offset past the end of the new text.

Cause: `apply_completion_item` replaces from `min(word start, item range start)` but passed only the word start to `completion_apply_snippet` as the snippet origin. Tab-stop offsets are origin plus a position inside the snippet text. When the item's range starts before the word (item 332 made the Blade range include the hyphen, `x-`; LSP items can also start before the word, for example at a `$` or `.`), the word start lies inside the replaced text. The transformer then mapped it to a point near the end of the inserted text, and adding the tab-stop offsets pushed the cursor beyond the end of the buffer.

Fix: `snippet_origin(word_start, edit_start)` is the real start of the replaced text and is used for both the selection and the snippet origin. As a safety net, `clamp_snippet_tabs` clamps every tab stop to the new buffer length before the cursor is set. The clamp does not snap to a character boundary; with the origin fixed that should not be needed, but the rope crate is not in the repo so I could not check the API.

Not covered: a safety net inside floem's `paint_normal_selection`. It lives in vendor/floem; the clamp at the source keeps the cursor valid, so I did not patch the vendored copy.

Tests: `snippet_origin_tests` (earlier start wins; tab stops are clamped).
