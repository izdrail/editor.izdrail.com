# Completion popup placement (item 312)

- Hidden when there are no filtered items. After item 311 gave the popup a border, the empty shell showed as a 1px line under the cursor.
- Flips above the cursor when it would pass the bottom of the editor viewport (previously the bottom of the whole tab, so it opened over the terminal panel). `should_flip_above` is pure and unit-tested. The existing clamp still keeps it inside the tab.

Capture needed: cursor on the last visible editor line with the terminal open; type a prefix with no matches.

## Item 360: stay inside the viewport and off the typed line

The popup flipped above the cursor whenever it did not fit below, even
when above had less room, and a popup taller than the space above was
then clamped to the window top edge, covering the line being typed
(seen on one-char PHP suggestions, where the list is tall and the
editor viewport short).

Placement now follows `popup_vertical_placement`: below the line when it
fits there, otherwise the side with more room, and the rendered height
is capped to the room on the chosen side (never below one line height,
so suggestions stay reachable). The popup therefore always ends at the
line top or starts at the line bottom instead of overlapping the text,
and it is never cut by the window edge. The final tab clamp uses the
capped height rather than the stale measured one, so a previously tall
popup can no longer be pushed back to y = 0.

The code action, rename, hover and overlay popups still use the old
flip-only logic; they are candidates for the same treatment.

Tests: `popup_vertical_prefers_below_when_it_fits`,
`popup_vertical_flips_above_only_when_above_has_more_room`,
`popup_vertical_never_covers_the_line`.

## Item 361: place by desired height, not the last measurement

Item 360 seeded the flip decision with the measured layout height. That
height is zero on the first open of a session (nothing rendered yet), so
a cold popup at the last visible line always "fit" below and was capped
to a one-row strip; after a capped render the measurement equals the
cap, which makes any placement self-justifying. Warm opens worked only
because `layout_rect` keeps the previous session's measurement.

Placement now uses the height the popup wants: filtered rows times the
line height plus the border, capped (`desired_popup_height`). The flip
decision no longer depends on anything the layout has rendered, so a
cold popup at the bottom line opens above at full size immediately.

Tests: `desired_height_counts_rows_plus_border`,
`popup_vertical_cold_open_flips_on_desired_height`.
