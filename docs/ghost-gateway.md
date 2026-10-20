# Authenticated ghost transport prerequisite (item 288)

App-side OpenAI chat-completions transport for short code insertions. Uses the
same endpoint-bound credential resolver as Chat (environment, saved, restricted
build fallback). No credential enters the serialized worker request. Redirects
and retries are disabled. Response is capped at 64 KB; output budget is 1-256;
requests time out in 1-120 seconds and poll cancellation every 25 ms. API errors
and malformed/fenced replies fail closed. Prefix/suffix are framed as code data,
not trusted instructions; delimiters are not a security boundary.

No caller/backend setting activates this prerequisite. Existing local and native
Ollama ghost requests remain unchanged until the shared connection patch. No
real gateway requests or keys are required to gate it.

Host: compile/test/clippy; gateway_ghost_* tests cover body, parser and pre-cancel.
Before activation, use a loopback HTTP fixture with dummy key to verify path,
Authorization header, no-redirect, status errors, oversized response, timeout and
in-flight cancellation. Real-model insertion quality remains unverified and
must be inspected with ghost acceptance/rejection in a running editor.

Item 289 wires this transport only after explicit shared Apply. It uses short
instruct completion, not native raw FIM tokens; model quality must be evaluated
for the user's selected ghost model, independently from Chat quality.

## Next-edit stale-response guard (item 291)

Each next-edit request advances the same generation used by cancellation and
ordinary ghost requests. Its callback rejects a changed generation, moved
cursor, non-insert mode, changed config or changed document/revision. An Escape
or newer prediction cannot resurrect a suggestion merely because file text did
not change. This applies to native Ollama and shared gateway next-edit.

Host: next_edit_prediction_rejects_cancel_replacement_and_cursor_move, compile/
test/clippy. With delayed dummy next-edit reply: cancel before completion, move
cursor, leave insert mode, then replace with a newer request. Each must suppress
the old reply; unchanged current request must still show and accept normally.
Capture resulting editor state for each interaction; source guards are not
visual acceptance.
