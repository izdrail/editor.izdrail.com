# Keymap presets and compact keyboards

Settings > Core > Keymap Preset selects `compact-65` (default), `vscode`,
`phpstorm`, `sublime`, `notepad-plus-plus` or `lapce` (unchanged upstream defaults).
The persisted setting is `[core] keymap-preset = "compact-65"` in settings.toml.
Changes follow the existing settings reload path; user keymaps.toml is loaded last
and remains the final override. Unknown preset strings fall back to compact-65.

Load order: common defaults, OS defaults, compact alternatives, selected profile,
user overrides. These are compatibility subsets, not full editor emulation. All
new editing bindings are Insert-mode and editor-focus scoped, so Vim Normal,
Visual and operator-pending editing and terminal controls are not overwritten.
Popup lists keep their navigation. File explorer rename/trash alternatives are
scoped to the explorer. For conventional editor behavior use Core > Modal = false.

## Switching from the palette

Command palette: **Switch Keymap Preset** (`palette.keymap_presets`; the palette itself opens with Ctrl+Shift+P or from the menu, no F-key involved). Lists every preset with a one-line description, marks the active one `(current)` and preselects it. Enter writes `[core] keymap-preset` in settings.toml, the same setting the Settings dropdown edits, and the existing settings reload applies it. There is no live preview: the keymap changes on Enter. An unknown value in settings.toml is shown as `compact-65`, which is what the loader uses.

## No Fn layer needed

All non-legacy presets include these Ctrl+Alt alternatives:

| Keys | Action |
| --- | --- |
| H / J / K / L | Left / Down / Up / Right |
| A / E | Start / end of line |
| U / D | Page up / down |
| B / F | Start / end of document |
| Backspace | Forward delete |
| W | Delete next word |
| N | Rename symbol (F2) |
| G | Go to definition (F12) |
| . / , | Next / previous error (F8 / Shift+F8) |
| X | Delete line |
| R in explorer | Rename selected file |
| Backspace in explorer | Trash selected file |

Add Shift to H/J/K/L/A/E/B/F to extend selection. Existing Ctrl+Left/Right word
navigation, Ctrl+Backspace, Ctrl+Tab and Shift+Ctrl+Tab remain available. Copy,
cut, paste, undo, save, open, new, select-all and find have Ctrl-letter bindings.
Home/End/PageUp/PageDown/Delete and function keys remain optional alternatives,
never the only route to essential editor navigation/editing. OS/global shortcuts,
AltGr behavior and non-US layouts may still require custom keymaps.toml changes.

## Compatibility differences

Profiles use sourced Windows/Linux-style Ctrl bindings. macOS retains existing
Cmd defaults plus the explicit Ctrl alternatives; these profiles do not emulate
JetBrains' separate macOS keymap. VSCode uses Ctrl+D select-next, Ctrl+L select
line, Ctrl+Shift+K delete line. PhpStorm uses Ctrl+D duplicate line, Ctrl+Y delete
line, Ctrl+Shift+Z redo and Ctrl+R Replace. Notepad++ uses Ctrl+D duplicate line,
Ctrl+Shift+L delete line, Ctrl+Y redo and Ctrl+Q toggle line comment. Ctrl+L is NOT
rebound to delete for Notepad++: upstream Notepad++ uses that for cut-line, and
Gravity has no exact cut-line command. Duplication maps to Gravity's line duplication,
not arbitrary selection duplication. Profiles leave unmapped base shortcuts in
place; this is an overlay, not a wholesale replacement.

Ctrl+H (VSCode/Notepad++) and Ctrl+R (PhpStorm) open Gravity's Replace control. The
existing focus_replace_editor handler is fixed to open a closed Find/Replace
control, instead of silently doing nothing.

No unavailable refactor, language-server, debug or folding commands are added.
Notepad++ reload/bookmark/macro features and PhpStorm IDE features are not emulated.
Commands, chord parsing, condition/mode rules and terminal key serialization still
live in Rust; bindings/presets and user overrides are TOML. The settings dropdown
and bundled preset names are Rust-defined. Some OS/framework text input behavior
is outside keymaps; keymaps are not a remapping of raw terminal escape sequences.

## Verified and remaining checks

Workspace cargo check passes. Four preset unit tests verify commands/keys,
Fn-free alternatives, overlay/user precedence and preserved modal maps. Visual
settings-dropdown inspection and interactive keyboard smoke tests are not done.
Before release check preset switching, Replace opening, Shift selection, popup
lists, terminal, explorer and OS shortcuts on the user's actual 65% keyboard.

Sources inspected:
- https://code.visualstudio.com/docs/reference/default-keybindings
- https://www.jetbrains.com/help/phpstorm/reference-keymap-win-default.html
- https://npp-user-manual.org/docs/editing/
- https://www.npp-user-manual.org/docs/searching/
- https://www.npp-user-manual.org/docs/user-interface/
- Notepad++ source clone: PowerEditor/src/Parameters.cpp at
  2ddf1a9cce8034b9b21d7d6af5f052262bb11cca (Ctrl+Shift+L deletes, Ctrl+L cuts,
  Ctrl+D duplicates selection, Ctrl+Q toggles comment).
- Fork defaults, keypress loader/matcher, settings reload and pinned Floem command
  implementation inspected locally at the 3ea06f14 base.

## Sublime Text preset

`sublime` (Settings > Core > Keymap Preset) maps Ctrl+P go to file, Ctrl+Shift+P
command palette, Ctrl+R go to symbol, Ctrl+Shift+R workspace symbol, Ctrl+G go to
line, Ctrl+D select next occurrence, Ctrl+L select line, Ctrl+Shift+D duplicate
line, Ctrl+Shift+K delete line, Ctrl+J join lines, Ctrl+Shift+Up/Down move line,
Ctrl+[ / Ctrl+] outdent/indent, Ctrl+/ comment, Ctrl+H replace. Multi-cursor,
bookmarks and Goto Anything suffixes (`:line`, `@symbol`) are not emulated.
This build was not compiled here (no Rust toolchain in the patch workspace); run
`cargo test -p lapce-app presets` before merging.
