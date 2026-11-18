# Shortcuts and keymaps

## Everyday actions

These are common shipped defaults. `Cmd` is written as `meta` in keymap TOML. Your selected preset or user bindings may replace them.

| Action | Linux / Windows | macOS |
| --- | --- | --- |
| Find a file | Ctrl+P | Cmd+P |
| Command palette | Ctrl+Shift+P | Cmd+Shift+P |
| Save | Ctrl+S | Cmd+S |
| Undo | Ctrl+Z | Cmd+Z |
| Redo | Ctrl+Shift+Z | Cmd+Shift+Z |
| Find in file | Ctrl+F | Cmd+F |
| Search workspace | Ctrl+Shift+F | Cmd+Shift+F |
| Go to line | Ctrl+G | Ctrl+G |
| Request completion (Insert mode) | Ctrl+Space | Ctrl+Space |
| Request signature (Insert mode) | Ctrl+Shift+Space | Ctrl+Shift+Space |
| Next / previous editor tab | Ctrl+Tab / Ctrl+Shift+Tab | Ctrl+Tab / Ctrl+Shift+Tab |
| Vertical split | Ctrl+Backslash | Cmd+Backslash |
| Settings | Ctrl+Comma | Cmd+Comma |
| Keyboard Shortcuts | Ctrl+K then Ctrl+S | Cmd+K then Cmd+S |
| Focus terminal | Ctrl+Backtick | Ctrl+Backtick |
| Previous / next location | MouseBackward / MouseForward | MouseBackward / MouseForward |

F12 requests Go to Definition, F2 requests Rename Symbol and F8 / Shift+F8 move through errors. Language-aware commands need matching language-server support. On compact keyboards, Ctrl+Alt+G, Ctrl+Alt+N and Ctrl+Alt+Period / Comma are alternatives supplied by the non-legacy presets.

## Presets and precedence

Load order is common defaults, OS defaults, compact alternatives, selected preset, then personal `keymaps.toml`. The `lapce` preset keeps the base defaults without compact overlays. Available profiles are `compact-65`, `vscode`, `phpstorm`, `sublime`, `notepad-plus-plus` and `lapce`. Set **Core > Keymap Preset** in Settings or use **Switch Keymap Preset** in the command palette.

The reference lists each layer separately, not a merged active-keymap table. A later binding can replace an earlier one. For example, do not rely on common-layer Ctrl+Minus for Back: the OS defaults also use it for Zoom Out. Use the side buttons or a personal binding instead.

Modes in the [full reference](shortcut-reference.md): `i` = Insert, `n` = Normal, `v` = Visual, `t` = Terminal; combined letters mean any listed mode. `all` has no explicit mode restriction. A condition must also match: a completion popup, search input or file explorer can take precedence. A space between keys means a chord, pressed in sequence.

## Add or replace a binding

Open **Keyboard Shortcuts** to inspect bindings and edit your keymap. The command palette also exposes the keymap editing commands. Personal bindings use TOML:

```toml
[[keymaps]]
key = "alt+left"
command = "jump_location_backward"

[[keymaps]]
key = "alt+right"
command = "jump_location_forward"
```

To remove a default side-button binding, add the exact key and command with a minus prefix:

```toml
[[keymaps]]
key = "MouseBackward"
command = "-jump_location_backward"
```

Pointer names are case-insensitive: `MouseBackward`, `MouseForward` and `MouseMiddle`. Modifiers are supported, for example `Ctrl+MouseBackward`. Removing a default and adding a modifier binding are separate entries. A `when` or `mode` on an unbinding must match the binding being removed. The default side-button bindings have neither.

## Compact keyboard movement

With a non-legacy preset, hold Ctrl+Alt and use H/J/K/L to move left/down/up/right, A/E for line start/end, U/D for page up/down and B/F for document start/end. Add Shift to H/J/K/L/A/E/B/F to extend selection. Ctrl+Alt+Backspace is Forward Delete. These editing alternatives are Insert-mode/editor-focus scoped, not terminal remaps.

OS/global shortcuts, non-US layouts and AltGr can still intercept a key. The [complete reference](shortcut-reference.md) records the shipped TOML; the active Keyboard Shortcuts screen and your own hardware are the final check.
