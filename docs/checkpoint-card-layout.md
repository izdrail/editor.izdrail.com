# Checkpoint card layout stays inside the panel (item 219)

Host QA on c59c3e9: the checkpoint card grew a horizontal scrollbar
whose thumb painted over the "Restore checkpoint" row and sat on the
Dismiss hit area. Two rows were wider than the panel: the one-line
"Checkpoint Applied: 1 file(s). Restart is review-only. Restore fails
on later changes." header, and the side-by-side Restore/Dismiss
button row.

The header is now two short lines ("Checkpoint Applied: 1 file(s)."
and "Restart is review-only. Restore fails on later changes."), and
the buttons stack vertically. No single row exceeds the panel width,
so the surrounding scroll never grows a horizontal bar. Standing rule
for this panel: every row in a scroll must fit the panel on its own -
the scroll measures content at infinite width, so labels do not wrap
inside it and wide rows produce the paint-over thumb.
