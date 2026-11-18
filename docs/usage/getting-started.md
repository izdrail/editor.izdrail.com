# Getting started

## Open the project, then a file

Open the project folder from the File menu. Opening the folder rather than just one PHP file lets the editor inspect Composer metadata, find files and detect framework-specific completions. Detection reads project files; it does not install or run a framework.

Press Ctrl+P on Linux/Windows, or Cmd+P on macOS. Type part of a file name, select a result and press Enter. Ctrl+G opens Go to Line. Enter a line number to move the caret. Ctrl+Tab and Ctrl+Shift+Tab switch between editor tabs.

## Choose an editing style

For ordinary typing, turn off **Settings > Core > Modal**. With modal editing enabled, keys depend on Insert, Normal and Visual mode. Press `i` in Normal mode to enter Insert mode. The shortcut reference records mode restrictions.

**Settings > Core > Keymap Preset** chooses the shortcut profile. The default is `compact-65`. Use the command palette's **Switch Keymap Preset** command to change it without opening Settings. Profiles are shortcut subsets, not full VS Code or PhpStorm emulation. See [Shortcuts](shortcuts.md).

## Save, undo and search

- Save: Ctrl+S, or Cmd+S on macOS.
- Undo: Ctrl+Z, or Cmd+Z on macOS.
- Redo: Ctrl+Shift+Z, or Cmd+Shift+Z on macOS. Presets may add or replace alternatives.
- Find in this file: Ctrl+F, or Cmd+F on macOS. Enter finds the next match; Shift+Enter finds the previous match while the search input has focus.
- Workspace search: Ctrl+Shift+F, or Cmd+Shift+F on macOS.
- Command palette: Ctrl+Shift+P, or Cmd+Shift+P on macOS. Search for a command by name if its shortcut is unfamiliar.

## Work across files

Ctrl+Backslash, or Cmd+Backslash on macOS, opens a vertical editor split. Side-button Back and Forward follow the existing file/location history, including the saved cursor position. They are not the same as cycling tabs. See [Mouse navigation](mouse-navigation.md).

## Request code help

In Insert mode, Ctrl+Space requests completion. Type more text to filter the list, use the arrow keys to select a result and Enter to insert it. Hover the metadata line to read a long qualified path/signature. Snippets and project-aware completions can work without a language server; semantic navigation and some signatures require one. See [Completion and keywords](completion.md).

The integrated terminal can be focused with Ctrl+Backtick. Commands typed there run in the workspace environment. AI features require their configured backend; a completion or edit request does not prove the backend is reachable. Review generated changes before saving or running them.
