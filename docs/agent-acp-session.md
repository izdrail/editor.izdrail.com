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

- fs **read** is advertised, fs **write** is not (the initialize message
  is patched after construction). `fs/read_text_file` is answered from
  disk, confined to the canonical workspace root, capped at 1 MiB before
  line slicing, with line/limit windows per the spec.
- `fs/write_text_file` is refused with a JSON-RPC error until the
  review/permission UI owns writes.
- terminal capability is never advertised.
- Agents that require authentication get `NeedsAuth`; the UI picks a
  method and calls `begin_auth`. On success the controller proceeds to
  `session/new` by itself.

## Events

`Connected, NeedsAuth, SessionStarted, Text, Plan, ToolCall, Usage,
FsRead, Notice, Error, TurnDone, Closed` - render in arrival order.
`Notice` is for tolerated oddities (unknown update kinds, stray session
ids); `Error` is for JSON-RPC error answers.

## Not here

UI wiring into the chat panel, a mode/selector, process watchdogs and
reconnect, permission prompts, write review. Those are the next
milestones; the panel milestone consumes `TurnEvent` directly.
