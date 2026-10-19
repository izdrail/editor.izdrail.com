# ACP session controller (`agent_acp_session.rs`)

Drives one hosted ACP agent (opencode natively; Claude Code/Codex via
their adapters) through the v1 handshake and prompt turns. Verified
against agentclientprotocol.com/protocol/v1 on 2026-10-17.

## Shape

Pure state machine. The caller owns the `AcpProcess` (transport) and
shuttles values both ways:

1. `AcpSession::start(workspace, name, version)` returns the controller
   and the `initialize` message to send. `workspace` must be
   canonicalized first; fs answers are confined to it.
2. Every decoded `AcpMessage` goes into `handle`, which returns
   `Vec<TurnEvent>` for the UI and `Vec<Value>` to write back to the
   agent's stdin (request responses, follow-up requests).
3. `begin_prompt(text)` (only while `Ready`) yields the
   `session/prompt` message; `cancel()` (only while `Prompting`) yields
   the `session/cancel` notification.

## Phases

`Initializing -> Starting -> Ready <-> Prompting`, plus `Closed` on an
unrecoverable protocol failure (bad version, missing sessionId, error
answer to initialize/session/new). Kill the process on `Closed`.

## Capability policy

- fs **read and write** are advertised by the session. `fs/read_text_file` is answered from
  disk, confined to the canonical workspace root, capped at 1 MiB before
  line slicing, with line/limit windows per the spec.
- `fs/write_text_file` stages proposed content for review; it does not
  change disk or buffers. Existing canonical workspace paths only,
  2 MiB per write and 32 paths per turn, last write per path wins.
  The panel drains proposals into the owner-review lane at turn end;
  only Apply changes buffers. See [write staging](acp-review-staging.md).
- terminal capability is never advertised.
- Agents that require authentication get `NeedsAuth`; the UI picks a
  method and calls `begin_auth`. On success the controller proceeds to
  `session/new` by itself.

## Events

`Connected, NeedsAuth, SessionStarted, Text, Plan, ToolCall, Usage,
FsRead, Notice, Error, TurnDone, Closed` - render in arrival order.
`Notice` is for tolerated oddities (unknown update kinds, stray session
ids); `Error` is for JSON-RPC error answers.

## Session continuity (item 143)

Agents advertising `loadSession` can resume a previous session:
`AcpSession::start_resumable` sends `session/load` instead of
`session/new`, the agent replays the transcript as `session/update`
notifications, and a load result without a sessionId keeps the resumed
id. Agents without the capability get a Notice and a fresh session -
the spec forbids calling session/load on them. `can_resume()` exposes
the negotiated capability.

`agent_acp_store.rs` persists ONLY the pointer per workspace
(`.lapce/acp/sessions.json`: session id, agent command, cwd, first
prompt line, updated stamp; 64-entry cap, temp-file + rename writes).
This store contains pointers and the first prompt line, not a full transcript.
The agent replays history on resume. This describes the ACP pointer store,
not a privacy guarantee about other editor history or run journals.
The panel provides a Resume control; see [chat panel](agent-acp-panel.md).

## Not here

This module remains a state machine, not a process or UI implementation.
The chat panel consumes `TurnEvent` and owns subprocess lifecycle and review
handoff. An ACP child is not sandboxed by these protocol capabilities;
its own filesystem and execution tools remain under its own permissions.
