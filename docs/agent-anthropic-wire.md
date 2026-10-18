# Anthropic wire adapter (item 202, parity item 6 first slice)

The hosted-providers config placeholders name Anthropic, OpenAI and
OpenRouter, all disabled. agent_wire already encodes the
OpenAI-compatible shape that an OpenAI/OpenRouter adapter needs; this
slice adds the other wire format named in
docs/agent-provider-protocol.md: Anthropic's Messages API.

`agent_anthropic` is pure encoding/decoding: no network, credentials
or spending. It exists so the transport slice (which the spend
contract gates on verified pricing, journal settlement and an explicit
enabled configuration) has a tested wire layer ready.

## Encoding (anthropic_request)

- System messages join into the top-level `system` field; the message
  list starts with the first user turn (a hard API requirement,
  enforced).
- Assistant tool calls become `tool_use` content blocks; tool results
  become `tool_result` blocks grouped into a single following user
  message, as the API requires.
- The loop's OpenAI-shaped tool schemas are rewritten to
  `{name, description, input_schema}`.
- Same bounds as the OpenAI encoder: explicit model, 1..=8192 output
  tokens, at most 32 unique tools, and the shared conversation
  validation runs before anything is encoded.

## Decoding (anthropic_reply)

- Only `stop_reason` `end_turn` and `tool_use` are accepted;
  `max_tokens`, `refusal` and provider error envelopes are rejected as
  incomplete or refused answers.
- The stop reason must agree with the presence of `tool_use` blocks,
  and the recovered calls go through the same structural validation
  against the registered tool names as every other adapter.

## Not in this slice

Transport, credential handling (env-var names only, never stored
secrets), price/capability discovery, usage settlement against the
budget journal, and any enable switch in Settings. Anthropic has no
OpenAI-style `tool_calls` finish-reason quirks to mirror; streaming is
not encoded.
