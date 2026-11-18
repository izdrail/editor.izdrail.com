# Mouse Back and Forward

Use the mouse's Back side button to return to the previous saved file/location. Forward returns to the next saved location. This follows the same global location history as the editor's navigation commands, not the order of open tabs. A location includes the file, cursor position and saved scroll offset.

1. Open a file with the file picker and move to a line.
2. Open another file, or jump to a definition.
3. Press the Back side button. The earlier file/location opens.
4. Press Forward to return. Reaching either end of history does nothing.

New navigation appends to the editor's existing jump history; this is not a browser-style recently used tab list. Ordinary typing does not create a separate file-history entry for every character. Unsaved buffer contents are not discarded by navigating away; navigation is not Save or Undo.

## Which buttons?

These are the extra physical side buttons often called buttons 4 and 5 by mouse vendors. The editor receives them as Back/X1 and Forward/X2. On Linux/X11, test tools usually number them 8 and 9; X11 buttons 4 and 5 are vertical wheel events. The wheel still scrolls and does not navigate files. Left, right and middle clicks keep their normal behavior.

The side-button press is captured before child tabs, editor text or panels can move the caret or activate controls. Navigation runs once on press, not again on release. It follows the active workspace's keymap dispatcher and alert guard. A blocking alert does not become a shortcut around confirmation. Other focused popups can still affect command availability.

## Customize

The shipped common bindings are:

```toml
[[keymaps]]
key = "MouseBackward"
command = "jump_location_backward"

[[keymaps]]
key = "MouseForward"
command = "jump_location_forward"
```

They have no mode restriction, so they work with ordinary editing and Vim-style modal editing. Personal keymaps load last. See [Shortcuts](shortcuts.md) for unbinding and modifier examples.

If the buttons do nothing, try the title-bar Back/Forward controls and inspect the active keymap. Mouse software or an OS-level remap can intercept side buttons before the editor sees them. This implementation uses platform Back/Forward events, not vendor-specific raw device numbers. Linux GUI verification does not establish behavior for every mouse driver or OS.

Implementation: [`mouse_navigation.rs`](../../lapce-app/src/mouse_navigation.rs), [`keymap.rs`](../../lapce-app/src/keypress/keymap.rs), and [`main_split.rs`](../../lapce-app/src/main_split.rs).
