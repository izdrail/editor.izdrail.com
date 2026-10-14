# Agent protocol and spend contract (milestone 2a)

This is a pure, tested protocol/configuration foundation. It does not yet add an
agent, execute tools, enable hosted requests or consume API credits. Existing
local/Ollama chat remains unchanged.

The tool-aware message contract has explicit user/system/assistant/tool variants,
registered tool names, correlated call/result IDs, bounded arguments and transcript
sizes. Missing/repeated results, duplicate IDs, misplaced system messages and
unregistered tools are rejected before another model turn. It is deliberately
separate from the answer-only Candle ChatML protocol.

AI Chat configuration has a hosted-providers array with disabled Anthropic,
OpenAI and OpenRouter entries. Each needs an explicit model, credential environment
variable name, nonzero per-request and lifetime limits (integer micro-US dollars).
No secret values are saved in TOML/transcripts. Keys do not grant spending authority.
The runtime integration will require an explicit enabled configuration and validate
credentials and a verified maximum price before making any request.

ChatGPT and Codex names do not mean browser subscription reuse. The first OpenAI
adapter should use authorized API access and an explicit model ID; Codex-specific
OAuth/CLI surfaces need separate verified contracts, not invented API endpoints.
Anthropic is a separate wire format from OpenAI-compatible transports.

SpendLedger reserves worst-case cost before IO and caps simultaneous reservations.
Unknown outcome/cancellation/missing usage charges the reservation; a crash keeps
outstanding reservations until explicit reconciliation. A provider cost above the
reserved bound blocks further requests. Local and Ollama paths need no paid ledger.

This pure ledger is NOT yet a working hard global spending cap. Before hosted
transport can be enabled, implement persistent journal storage, exclusive cross-tab
and cross-process locking, verified price/capability discovery, per-request input/
output/tool ceilings, reconciliation and crash recovery. A user-supplied price guess
cannot establish an upper cost bound. Do not release a reservation when a cancelled
server could still bill it. Avoid retries unless their cost is separately reserved.
Provider-side hard key caps are an additional guard, not a replacement for editor
limits. Limits here are lifetime/non-resetting; daily/monthly reset semantics are
not yet implemented or advertised.

Sources consulted for future transport:
- https://docs.ollama.com/capabilities/tool-calling
- https://openrouter.ai/docs/api_reference/authentication
- https://openrouter.ai/docs/guides/best-practices/spend-controls

Verification: seven pure contract/ledger regression tests. No authenticated hosted
endpoint or billing behavior was tested. Milestone 2b is actual tool-aware local/
Ollama transport and durable locked budget journal; optional paid adapters remain
off until those checks are complete.
