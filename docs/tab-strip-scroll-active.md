# Tab strip scrolls to the active tab (item 327)

With many tabs the active tab was not brought into view. The strip's `ensure_visible` follows each tab's `layout_rect` signal, but nothing ever wrote that signal (the tab's `on_resize` only stored its size), so the target stayed `Rect::ZERO`. The tab's `on_resize` now also sets `layout_rect`, and `ensure_visible` reads it tracked, so the strip scrolls once layout reports the rect and again when the active tab changes or moves.

Capture: 17 tabs, activate the last one (also via the file explorer or palette): the strip scrolls to show it.
Not covered: the modified dot was not visible only because the active tab was off-screen; recheck after this lands. The Open Editors list height (4 rows with a scrollbar) is a separate item.
