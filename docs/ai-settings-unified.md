# One AI settings page (item 287)

AI Chat and AI Autocomplete now share one AI sidebar target and scroll section.
Chat and ghost-text feature/model rows retain their descriptive prefixes. Old
navigation requests for either section resolve to AI. Underlying ai-chat and
ai-autocomplete config keys are unchanged, as are saved values and runtime
routing. Opening this page does not write settings.

This is the page consolidation step, not a shared-transport migration. A routing
notice shows each backend. Legacy ghost backend/endpoint controls are behind
Show advanced legacy ghost connection. They remain available for existing
key-less Ollama profiles. Chat's connection/key does not yet authenticate ghost
requests; a working shared gateway transport and explicit Apply are follow-ups.

Host: compile/test/clippy including legacy_ai_navigation_targets_share_one_section.
Capture normal/narrow sidebar (one AI entry), top card wrapping, advanced closed
and open, and scroll down through both feature/model rows with no overlaps.
Test old Chat and Autocomplete settings links land on AI, search still finds
both legacy prefixes, and editing each row writes its original table. With a
mixed saved profile, open/close and restart must preserve both backend/endpoint
choices byte-for-byte. No send, automatic routing migration or real key is needed.

## Explicit shared connection (item 289)

One pending Local/Remote choice and endpoint/key live in the shared card. Apply
writes backend, endpoint, Chat model and shared-connection=true in one settings
file write. Only that click activates shared routing; no opening-time migration.
Chat and ghost enabled toggles remain separate feature toggles on the same page.
Chat/ghost models stay separate. Ghost remote model must be supported by the
selected OpenAI endpoint; there is no silent model fallback. Existing native
Ollama ghost fields remain stored for advanced restore. Workspace overrides may
still take precedence over user-file changes and the current-routing notice is
the authority for effective configuration.

With shared active, Chat local means ghost local; Chat remote means app-side
OpenAI ghost using the same endpoint/auth. Next-edit uses the same transport
when explicitly enabled. Automatic ghost triggers send bounded cursor context;
the card warns before Apply. Local selection never falls back to remote.
Credentials never enter worker requests. Config changes discard stale results;
new requests and explicit cancel abort the previous gateway request. Advanced
Restore removes the shared override, not the preserved legacy fields.

Host gate: shared_ai_is_explicit_and_preserves_legacy_values; full compile/test/
clippy. Dummy loopback HTTP fixture before real use: explicit Apply remote must
route Chat and ghost to /v1/chat/completions with dummy bearer; local Apply must
produce zero requests; opening a mixed legacy profile must produce no writes or
route changes. Verify manual/automatic ghost, next-edit opt-in, cancellation,
stale doc/cursor/config rejection, suffix de-duplication, accept/reject and two
model fields. Capture normal/narrow shared choice, disclosure, feature toggles,
advanced restore and fresh-local sidebar. Model output quality and remote quota
are not established by parser tests. No real project code or key in fixtures.

## Unknown legacy backend guard (item 290)

Before shared Apply, only the legacy explicit `ollama` value may select remote
inference. Every other stored ghost backend value, including `openai`, remains
local. Preserve the raw stored value; normalize only the effective routing.
This prevents the new internal lane name from reinterpreting an old unknown
value as remote approval. Host: unknown_legacy_backend_cannot_activate_gateway,
then a loopback fixture with saved openai and shared=false must see zero requests.

## Pending backend wording

The connection card shows "Pending backend: Local" or "Pending backend: Remote". Internal backend ids (for example "ollama") are never shown.

After Apply, the Settings page stays on the AI section instead of resetting to Core (the section is pinned to AI; the row rebuild no longer drops the view to the top). Exact scroll offset inside AI is restored to the AI header.
