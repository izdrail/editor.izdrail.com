# Agents row: Delete stays whole (item 315)

In a narrow panel the saved-session row's "Delete" was clipped to "Del". The title now has `flex_basis(0)` with `min_width(0)` and an ellipsis, so it gives up width first; Delete has `flex_shrink(0)`. The row and the list (now an explicit column, `width_full`, `min_width(0)`) are bounded by the panel width instead of growing to the title's natural width.

Capture needed: narrow right panel with a long session title; Delete fully visible, title ellipsised; also several sessions stacked vertically.
