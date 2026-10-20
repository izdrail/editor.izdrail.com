# Keymap-aware ghost text hint (item 277)

A compact status-bar hint appears only for an active inline suggestion in the
active document with Workbench focus. It resolves accept/dismiss sequences from
loaded keymaps, checks mode/conditions and omits ambiguous exact bindings or
chord prefixes. It never promises unconditional Tab/Esc. If either binding is
unavailable the entire hint hides. User-keymap reload changes the labels.

The hint is conservative: ambiguous bindings that could fall through to the
inline command are omitted, not predicted. No command, request, acceptance or
ranking semantics change. Narrow status bars may elide the hint; popup/overlay
placement is intentionally avoided so code stays unobstructed.

Host checks: active suggestion in the correct file, default and custom remapped
accept/dismiss keys, unbound keys, conflict/prefix, normal vs insert mode,
palette/search/terminal focus. No hint for stale suggestion from another file.
At 900px check status overflow alongside diagnostics/cursor info. Accept/dismiss
must follow the actual displayed binding. Pixel capture latency already observed
on the host is not treated as proof of an editor focus defect.

Edge case: the hint also requires that ghost text is currently drawn in the document. A session can stay Active after typing or a cursor move hides its text; the hint then hides instead of advertising keys for nothing.
