# Bindings for F-key-only commands (item 325)

For keyboards without a function row. Added: Alt+Shift+N / Alt+Shift+P for next/previous problem (F8 / Shift+F8), and Ctrl+Shift+R (Cmd+Shift+R on macOS) for file explorer rename while the explorer has focus (F2). Editor rename already uses the same chord with `!file_explorer_focus` (item 321), so the two do not overlap. F1 already has Ctrl+Shift+P; Ctrl+F4 (close split) and Alt+F4 (close window) are left as they are. A grep of all three keymap files found no existing use of these chords. Check Keyboard Shortcuts for conflicts after landing.
