# Shared placement for line-anchored popups (item 363)

Code action, rename and the inline AI edit overlay now place their
popups through the same `popup_vertical_placement` the completion popup
uses (item 360), via a shared `line_popup_y` helper: below the anchor
line when it fits, otherwise the side with more room, and always inside
the editor viewport rather than clamped to the window top edge. The old
code flipped above whenever below was full, even when above had less
room, and a too-tall popup could be pushed over the tab bar.

Unlike completion, these popups place by their measured height: their
views keep a static (or no) height cap, so a measurement cannot make a
placement self-justifying, and their content height is stable. The
completion-specific cold-open fix (item 361, desired height) stays
completion-only; a cold code action at the bottom line can still render
one frame below before the measured flip, as before.

Placement math is covered by the existing `popup_vertical_*` tests; the
wiring adds no new logic.

Capture needed: code action (and rename) opened with the cursor on the
last visible editor line, terminal panel open - popup lands above the
line inside the viewport.
