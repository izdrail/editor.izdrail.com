# Rename and inline AI edit placement (item 320)

Same defect as completion (312) and code actions (319): both overlays flipped against the whole tab height, so near the editor bottom they opened over the terminal, and were not clamped. They now use `should_flip_above` against the editor viewport bottom and `clamp_popup_origin`. Capture: F2 rename and the inline AI edit shortcut on the last visible line with the terminal open.
