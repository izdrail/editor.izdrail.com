# Wire-reply parsing regression tests (item 220)

The OpenAI-compatible reply parser (agent_wire::reply) decides whether
a provider response becomes an Act step or an error. It now has direct
tests for the edge cases host QA kept hitting:

- finish_reason "length" returns the actionable token-limit message
  (regression coverage for item 217).
- A provider "error" object and a missing finish_reason are rejected
  with their exact messages.
- Tool arguments as a JSON object (not only a stringified object) are
  accepted; a non-string message content is rejected.
- finish_reason must agree with the presence of tool calls in both
  directions ("stop" with calls and "tool_calls" without calls are
  both rejected before any conversation validation).
