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

## End-to-end Act wire tests (item 234)

The parser tests above cover replies in isolation; two loopback
tests now cover the whole conversation over HTTP:

- act_wire_round_trip_tool_call_then_proposal: run() drives a real
  agent_transport::ollama against a loopback server. The first
  request is asserted to carry the system contract (with the
  worked example naming the open file) and the tool definitions; a
  tool_calls reply triggers the host executor; the second request
  is asserted to carry the tool result as a role:tool message keyed
  by the provider's call id. The run finishes with the proposal.
- act_wire_bad_answer_triggers_repair_request: a prose first answer
  produces a second request containing the repair message ("not a
  valid Act proposal") instead of a run failure.

Both use a content-length-aware request reader: an Act request with
the contract and tool definitions runs several KB, past a single
read.
