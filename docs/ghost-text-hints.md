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

## Borrow panic fix (item 302)

0301 made the hint effect depend on the doc's ghost text. `InlineCompletionData::update_doc` sets that text while the data signal is mutably borrowed, so the hint effect ran inside the borrow, read the data signal, and panicked ("RefCell already mutably borrowed"). The three call sites that update the data and draw the doc together (request results, next-edit, refresh on cursor move) now go through `inline_completion::update_batched`, which batches the update so effects run after the borrow ends.

## Stale ghost text with auto-trigger off (item 303)

With `[ai-autocomplete] auto-trigger` off (the default) a ghost suggestion comes only from Alt+Backslash. The edit handler for typing returned early for that case, before the code that re-checks the visible suggestion, so a mismatching character never hid it (the typed character then appeared after the ghost text, because the ghost is drawn at the position where it was requested). The early return now calls `refresh_visible_inline_completion`: no request is made, but the visible suggestion is hidden on mismatch or shortened on a matching prefix, and cancelled when it belongs to another line or file.

## Typed characters (item 304, supersedes the 0303 routing)

0303 added the refresh inside `request_inline_completion`, but a typed character never reaches that function when auto-trigger is off. The real chain is `receive_char` (or `run_edit_command`) -> `update_inline_completion(Automatic)`, which only armed the debounce timer when auto-trigger was on, and the old refresh lived behind the timer even then. `update_inline_completion(Automatic)` now asks the pure `automatic_edit_plan` and always refreshes a visible suggestion immediately; only scheduling a new request depends on auto-trigger.
