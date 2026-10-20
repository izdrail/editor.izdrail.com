# Send selected code to chat (item 284)

Right-click inside a nonempty selection offers Send to chat. It snapshots the
selected code, file path (Untitled for scratch) and 1-based inclusive line range,
opens Chat and appends it to the composer without sending a request. Multiple
selected regions get separate range labels. Existing draft text is retained.
Right-click outside selection keeps existing behavior of moving the caret.
Selections above 32 KiB omit the action; no silent truncation. Context delimiters
are readable labels, not a security boundary or escaping mechanism.

Host: normal/narrow context menu and resulting focused composer, saved/scratch,
single/multiline/multiregion selection, existing draft, no selection, >32KiB.
Verify exact bytes/path/range and no network request until explicit Send/Enter.
Selection text is untrusted code context, not an instruction from the user.
This adds draft context only; normal Send may additionally attach active editor
context through its existing behavior. Verify panel hidden and already open.
