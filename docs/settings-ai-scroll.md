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
