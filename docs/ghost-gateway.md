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
