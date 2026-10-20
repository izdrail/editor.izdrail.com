# Completion popup edges (item 311)

The popup had a rounded corner radius but no border, shadow or inset, so rows touched the edge and the selected row's background squared off the corners. It now has a 1px `LAPCE_BORDER` border, the dropdown shadow used by other floating panels, 3px horizontal inset, and 3px radius on the kind badge and label so selection follows the curve. Vertical metrics are unchanged, so row offsets and `ensure_visible` are unaffected. Width is still `completion_width` (the 1px border sits inside the layout rect).

Needs a running-editor capture: popup over text, over a dark and a light theme, selected row at first and last item.
