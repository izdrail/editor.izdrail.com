# Focus trace (item 276)

Debug build only, off by default. Run in a disposable profile with dummy key
text: `LAPCE_FOCUS_TRACE=1 <debug-binary> 2>focus-trace.log`.
Capture a short sequence: click masked field, type one dummy character, Tab,
wait without other input, Shift-Tab, wait, Shift-Tab, type a dummy character.
Send only lines beginning FOCUS_TRACE, plus matching screenshot timestamps.
Do not forward the full stderr/log: other existing diagnostics are outside
this trace's sanitization contract. Stop the process and remove the variable
after the test. Release builds contain no trace module or call sites.

Records include key-event sequence, logical kind (all character values redacted),
physical key code, modifiers, before/after focused view IDs, queued focus changes,
paint update/redraw requests and paint begin. Native unidentified key values are
redacted too. It never prints event Debug, key text, clipboard, view debug names,
request contents, provider responses or saved credentials. Physical codes still
reveal which keys were pressed, so use dummy input only, never a real secret.
Events outside the key handler can carry the last sequence ID; stage labels
explain that ordering, not a claim they were synchronously caused by that key.

This patch changes diagnostics only. It must not be reported as a focus fix.
Host must verify debug compilation and collect actual key/paint ordering.
