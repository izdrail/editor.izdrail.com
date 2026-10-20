# Chat instruction readability (item 278)

Empty-state instruction and composer context/loading guidance use foreground
rather than disabled-looking dim text. They wrap at narrow widths. Loading copy
now distinguishes ACP/local/remote rather than always claiming local replies.
No timeout, backend, data-sharing or cancellation behavior changes.

Host normal/narrow/600px checks: read context and Enter/send instructions,
local vs remote vs ACP loading text with Stop, empty-state wrapping, and scroll
to composer without overlap. Color choice is not a measured contrast-ratio pass;
theme-specific pixels still need review. No gradient/fade code was found in
these views or the scroll component. Prior nested viewport edge clipping was
removed in 0258; any remaining fade requires a current capture, not an invented
CSS/gradient fix. Ordinary outer viewport clipping is expected while scrolling.
