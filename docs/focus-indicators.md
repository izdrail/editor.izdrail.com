# Focus indicators: panel rings, composer ring, terminal cursor (item 173)

Focus is now visible in three places:

- **Focused panel**: the chat and terminal panels draw a 1px ring in
  `editor.focus` when their `Focus::Panel(..)` is active; the ring is
  drawn in the panel background color otherwise, so nothing shifts
  layout when focus moves. Other panels follow the same one-line
  pattern when their turn comes.
- **Composer card**: the ring switches from `lapce.border` to
  `editor.focus` while the composer editor holds floem focus, driven
  by FocusGained/FocusLost listeners on the composer. Those listeners
  also set `Focus::Panel(PanelKind::Chat)` - floem focus events fire
  even where the editor stops pointer propagation, which makes them
  the reliable hook.
- **Terminal cursor**: unchanged code, now reachable - the hollow
  (stroked) cursor when unfocused already existed in
  `terminal/view.rs`, but the chat panel never set the app focus
  model, so `Focus` stayed on `Panel(Terminal)` and the cursor stayed
  solid. With the composer now claiming focus on click, the terminal
  cursor goes hollow as designed.
