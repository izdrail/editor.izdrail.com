# Clickable markdown links

Markdown links in hover popups, chat messages and plugin details
open in the system browser on click (item 443).

## How it works

`parse_markdown` records each link's byte range inside its text
block (`MdLink`). The shared `linked_text` view renders the block
and, on pointer-down, hit-tests the click position against the
block's link ranges (a private layout copy resized to the view's
width, so wrapping matches) and opens the URL with the system
handler. Plain-text conversions carry no links.

## Limits

There is no hover affordance yet - the pointer does not change over
a link. A link whose block is re-wrapped between clicks still hits
correctly because the hit layout is re-sized from the view's actual
width at click time.
