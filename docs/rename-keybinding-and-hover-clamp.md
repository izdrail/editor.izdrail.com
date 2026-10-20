# Rename shortcut without F-keys, hover clamp (item 321)

- Rename Symbol was bound to F2 only. Added Ctrl+Shift+R (Linux/Windows) and Cmd+Shift+R (macOS), same `!file_explorer_focus` condition, for keyboards without a function row. Check Keyboard Shortcuts for conflicts after landing.
- The hover popup falls back to below the cursor when it does not fit above, which could run past the tab bottom. It is now clamped with `clamp_popup_origin`.
