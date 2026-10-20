# Set up the LLM connection (item 253)

Open Settings, select AI Chat, and use **LLM connection** above the general
settings. Endpoint defaults to https://llm.izdrail.com and model to `auto`.
Enter the provider API key in the masked field and choose Save connection.
The key is never displayed, copied by the field or put in a general editor
buffer. Entry supports typing, paste, Backspace and Escape (clear draft).
Empty entry preserves the saved key; Remove saved key explicitly deletes it.
Settings file changes are picked up by the existing config watcher.

Keys are stored by normalized completion URL in `llm-credentials.json` inside
the app config directory, separate from settings.toml and Debug serialization.
This is local plaintext, NOT an encrypted OS keychain. Unix files are created
0600 and replaced through a temporary file; other systems use their inherited
filesystem ACLs. Do not share/commit the file or a config-directory archive.
An invalid file is not silently overwritten. No real key belongs in tests.

LAPCE_LLM_API_KEY remains an environment override and takes precedence over a
saved key. Keys saved for one endpoint are not used for another endpoint.
Authenticated requests do not follow redirects. All chat, model-list,
capability-probe and tool-agent requests use the same credential selection.
401/403 guidance points here rather than asking users to set an env variable.

Test connection sends an explicit one-token generation request using the
entered endpoint/model and saved key (or env override). Save a draft key
first. It sends no project code but can use provider quota. Status reports OK,
401 Unauthorized, 403 Forbidden, another HTTP code, or network failure without
showing request details or provider response bodies. It verifies acceptance,
not model quality or later streaming/tool capabilities.

Verification: fixture-only credential persistence/isolation/permissions tests,
input bounds, override precedence and status messages. Real settings layout,
focus/paste workflow, live gateway and config reload need host-run checks.

## Save failure integrity (item 257)

Temporary-file cleanup starts only after this save successfully creates the
file. A stale pre-existing temporary file or symlink causes a safe error and
is not deleted. The previous credential store stays unchanged. Malformed
stored JSON is also preserved, not silently reset. This patch changes no UI;
the running settings screen still requires the UI regression checklist.

## Connection status and test freshness (item 261)

The connection view reports the effective key source for the endpoint in its
field: environment override, saved endpoint-bound key, none, or unavailable.
No key bytes or credential-file parser details are displayed. An unsaved key
stays masked and must be saved before testing. Environment status takes
precedence even when the local credential file is unreadable.

Tests use the current endpoint/model fields, which may not yet be saved.
A completed result is only current while those values and the view's credential
save/remove generation still match. Editing a field or entering a new key marks
it stale. Editing while a test is in flight must not attach its success to the
new values. This is not continuous credential monitoring: changes made outside
the view require reopening it and retesting.

Host regressions: at normal and narrow width, capture no-key and environment
source states using fixtures only. Enter a dummy key and confirm no character
is painted. Save/remove it and verify source refresh. Test a fixture server,
change model while in flight, and require the stale-result message. Test again
without edits and require the result for the current fields. Check 401 and
20-second failure feedback, wrapping and vertical scrolling with Terminal open.
Do not send a real provider request merely to validate screenshots.

## Masked field focus (item 267)

The custom masked key field now paints a focus-colored border while focused.
Its key listener lets Tab/Shift-Tab continue to framework focus navigation
instead of unconditionally swallowing them. Secret-entry keys keep the prior
append/backspace/paste/Escape behavior; no actual key text is painted. Placeholder
and masked-entry labels wrap at narrow widths without shrinking the field.

Host checks: click focus, type dummy text, confirm only fixed dots and the
unsaved label; Tab and Shift-Tab must leave the field, border must reset, and
subsequent typing must not append to its key. Return and check Backspace,
paste and Escape. Repeat at 900px with Terminal open. A debug prefill screenshot
checks masking only and cannot replace these keyboard tests. No claim is made
that arbitrary cursor editing/selection is supported by this append-only field.

## Explicit key focus handoff (item 268)

0267's event propagation alone did not move focus on the host: the window root
handles Tab as a workbench key. Key PointerDown now requests focus immediately,
updates shared input focus and sets Workbench routing rather than leaving the
Terminal as the command target. Empty focused entry has a visible focused hint.
Tab explicitly hands focus to Save connection and clears the field's ring.
Control/Meta shortcuts other than paste continue to the workbench dispatcher.
Shift-Tab currently also hands off to Save (not reverse traversal); general
reverse navigation remains a limitation. Neither Tab nor shortcuts save a key.

Re-test click-before-type ring, real dummy typing, Tab ring reset, command
palette open from the field, then chat composer versus terminal typing. Inspect
both shared focus and actual destination; no runtime fix is claimed until then.
