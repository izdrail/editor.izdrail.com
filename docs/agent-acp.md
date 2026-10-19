# ACP client foundation (agent-acp)

`agent_acp` is the pure protocol contract for hosting Agent Client Protocol
(ACP v1) agents in the editor: opencode natively (`opencode acp`), Claude
Code and Codex CLI through their ACP adapters. It provides:

- NDJSON stdio framing (one JSON-RPC message per line; encode asserts the
  single-line invariant, decode tolerates one trailing CRLF and rejects
  embedded newlines)
- Client request builders: `initialize` (capabilities explicit; the session controller advertises
  workspace-confined reads and review-staged writes, with terminal false),
  `authenticate`, `session/new`, `session/prompt`, `session/cancel`
- Agent response/notification parsers: initialize hello (protocol version,
  agent info, auth method ids, loadSession), session id, stop reason,
  `session/update` (message chunks, plans, tool calls, usage), and the
  agent-to-client `fs/read_text_file` / `fs/write_text_file` requests

Shapes verified against agentclientprotocol.com/protocol/v1 on
2026-10-17 (transports, initialization, session-setup, prompt-turn,
file-system pages).

This module does NOT spawn processes, touch stdio, read or write files,
apply edits, grant permissions or render UI. Those integrations arrive
separately; an agent process must never gain filesystem or execution
ability from this module alone. `agent_acp_process` is the transport half: launch the agent binary with
exact program/args/cwd (no shell, no lookup), send/recv NDJSON frames
(blocking; run off the UI thread with an external watchdog), kill/reap.
stderr drains on a background thread into a 100-line ring so a chatty
agent cannot deadlock on a full pipe; lines over 1 MiB are errors. Tests
roundtrip a frame through a real `cat` subprocess and cover EOF (`true`)
and spawn failure. The chat-panel session surface, context passing and
permission-gated fs handlers are implemented by the session and panel layers.
See [session controller](agent-acp-session.md), [chat panel](agent-acp-panel.md)
and [write staging](acp-review-staging.md) for the current behavior.

Verification: unit tests cover framing roundtrips and newline rules, the
spec-example request shapes, hello/session/stop parsing, plan/tool-call/
usage updates and fs request parsing. These unit tests do not prove a live authenticated run with any adapter.
