# AI settings scroll containment (item 254)

The auxiliary AI connection/model pane now follows explicit sidebar selection,
not the scroll-derived current heading. Previously crossing a heading could
show/hide a fixed-height pane, changing the remaining viewport while the user
was scrolling it. This feedback could look like jumps or mixed sections.

Its inner scroll no longer propagates wheel events at its bounds. Scroll over
the general settings list to move that list; select AI Chat or AI Autocomplete
in the sidebar to open the corresponding auxiliary pane. Merely scrolling past
an AI heading no longer opens that pane automatically.

Code-grounded hypotheses, not a confirmed diagnosis from the screenshot alone.
Host UI check: repeatedly wheel over both panes and at each end; sidebar
selection must not jump repeatedly; credential controls must remain readable.

## Superseded layout: item 258

The fixed-height auxiliary pane is removed. Connection setup and all local
model rows now live immediately under the AI Chat heading inside the ordinary
settings scroll, and never appear above AI Autocomplete. There is one settings
content viewport; no inner model-scroll boundary can cut a different subsection
or feed wheel events into a second settings list. Model explanations use the
normal foreground color, not a disabled-looking dim color. Endpoint/model
inputs have visible borders and connection action rows wrap at narrow widths.

Visual acceptance requires new normal/narrow screenshots plus wheel/focus
checks. Earlier captures showing the fixed-height pane do not verify this change.

The generic AI Chat endpoint/model rows are omitted: the connection editor is
the only editable surface for those settings. This avoids two simultaneously
buffered values disagreeing after model selection. Settings-file edits still
reload through the existing watcher. Reopen settings after outside edits;
live reconciliation of an already dirty connection draft is not implemented.
