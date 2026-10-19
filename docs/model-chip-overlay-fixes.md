# Model chip overlay fixes (item 208, host QA follow-up)

Two bugs from the first host render of the endpoint model list:

- The "Loading models..." row stayed visible as the first row after
  the list loaded. Root cause: the dropdown's dyn_stack keyed rows by
  their enumerate index, so key 0's view was reused when the loading
  row became the first model - the reused view kept the old moved-in
  label. Rows now key by their own id text, so a vanished row's view
  is dropped instead of recycled.
- The overlay could render past the window bottom. The shared
  composer_dropdown_overlay already scrolls above 220px and flips
  upward when it would overflow, but a stale measurement or a click at
  the very bottom edge could still push it out; the computed position
  is now clamped inside the window on both axes (both dropdowns gain
  the clamp).
