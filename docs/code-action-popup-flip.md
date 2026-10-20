# Code action popup flip (item 319)

Same defect as completion (item 312): the code action menu flipped only when it passed the bottom of the whole tab, so near the editor bottom it opened over the terminal, and it was never clamped. It now uses `should_flip_above` against the editor viewport bottom and then `clamp_popup_origin`, both shared with completion and unit-tested there. Capture: Ctrl+. on the last visible line with the terminal open.
