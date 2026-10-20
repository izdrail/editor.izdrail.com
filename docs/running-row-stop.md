# Running rows keep Stop visible (item 316)

Same defect as the Agents row (item 315): the Running row's title label had its natural width and pushed Stop off the edge in a narrow panel. Title now shrinks with an ellipsis (`flex_basis(0)`, `min_width(0)`), Stop has `flex_shrink(0)`, and the list is an explicit column. Capture: start two runs in a narrow panel.
