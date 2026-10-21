# Tab strip scrolls to the active tab (items 327, 329)

With many tabs the active tab was not brought into view.

Item 327 was wrong about the cause. It claimed `layout_rect` was never written. It is: the outer tab stack's `on_resize` sets it to the tab's rect inside the strip. Item 327 also added a second write from the inner (draggable) tab view, whose rect is relative to its own parent (x near 0). That write overrode the real rect, so the strip saw the tab as "visible" and did not scroll. Item 329 removes that write.

The real defect is in the strip's `ensure_visible`: it read the rect with `get_untracked`, so the effect ran once, at creation, when the rect was still `Rect::ZERO`, and again only when the active index changed. It now reads the rect tracked (kept from 327), so it re-runs when the first layout reports the rect, and whenever the active tab moves or resizes.

Capture: 17 tabs; on launch the active tab (far right) should be in view; then switch tabs via the explorer and the palette.
