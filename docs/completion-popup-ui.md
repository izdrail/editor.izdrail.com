# Completion popup containment (item 266)

The list already scrolls the active row into view. Its configured width and
400px height limit now also respect the owning tab's width/height. Above/below
placement ends with a nonnegative, in-tab origin clamp, including short tabs
where flipping above the cursor would otherwise yield a negative Y. Wheel
scrolling does not propagate into the editor at list bounds.

No ranking, request, ghost text, keybinding or acceptance semantics change.
Default inline suggestions use Tab/Esc in insert mode with focus exclusions,
but user keymaps can override them; unconditional painted hints are avoided.

Host matrix: populated list at all four edges, 900px/normal width, Terminal open,
short editor height, a configured completion width larger than the tab. Arrow
through more than one page and require the selected row visible. Wheel at both
ends must not scroll editor underneath. Click item, dismiss, reopen and check
no pointer/focus regression. Capture long PHP symbol names and detail labels;
text elision/tooltip decisions await those actual pixels. Test custom keymaps
before any future accept-hint UI. Pure geometry tests are not visual acceptance.
