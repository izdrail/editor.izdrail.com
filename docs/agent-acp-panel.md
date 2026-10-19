# ACP chat panel wiring (item 135)

The chat panel gains an ACP mode: the `ACP` button next to Chat/Ask/Plan
routes Send to a configured external agent subprocess over the Agent
Client Protocol, instead of the built-in chat or the Ask/Plan loop.

## Setup

The composer mode dropdown offers "Agent: opencode", "Agent: Claude Code"
and "Agent: Codex". Picking one sets `[ai-chat].acp-command` in settings.toml,
updates live config and selects ACP mode. See [agent picker](acp-agent-picker.md).
For a custom command, `acp-command` is the setting that starts an ACP
agent, for example `opencode acp` (native) or a Claude Code/Codex
adapter binary. Empty (the default) disables ACP mode with a hint.
Whitespace splitting only; keep the binary path space-free.

## Behavior

- One Send spawns a fresh agent process rooted at the current project
  folder (canonicalized). Session continuity across sends uses `session/load` when supported;
  see Resuming sessions below.
- The reply body shows setup status lines, then the streamed agent
  message chunks, plan entries, tool call titles, fs-read notices and
  usage updates, separated by blank lines.
- Agents can read project files through the session controller
  (workspace-confined, 1 MiB cap); file writes are staged for owner review,
  not applied directly (see [write staging](acp-review-staging.md)). The agent's own tools still run under the agent's own
  permissions - ACP mode is for agents you already trust, same as
  running them in a terminal.
- Stop kills the agent subprocess by pid (Linux `kill`), because the
  worker thread is usually parked in a blocking read.
- Authentication: if the agent asks for it, the turn stops with
  instructions to authenticate in a terminal once; the panel does not
  run OAuth flows itself.
- Linux only, like Ask/Plan.

## Files

- `config/chat.rs`: `acp_command` setting (kebab-case `acp-command`).
- `panel/chat_view.rs`: mode button, send routing, `send_acp` +
  `run_acp_turn` + `drive_acp_turn` + `render_acp_event`, Stop kills pid.
- `agent_acp_process.rs`: `id()` pid accessor.
- `agent_acp_session.rs`: `split_command` helper (+ tests).

## Editor context (item 137)

Each Send attaches the active file or selection to `session/prompt`,
capped at 12 KB by the same `chat::Context` plumbing the chat backend
uses and wrapped in the same `<editor_context>` fence (untrusted data,
one convention across AI surfaces). The turn's context line shows the
indicator (`Selection: path | N bytes (truncated)`) next to the agent
command, and the panel's "On Send" line previews it.

This context leaves the editor: the agent subprocess forwards it to
whatever model IT is configured to talk to. Only use ACP mode with
agents and models you trust with the file you have open.

Diagnostics are not attached yet; resource_link content blocks (instead
of fenced text) come once adapter support is verified.

## Resuming sessions (item 145)

A `Resume (N)` button sits next to ACP in the mode row; N counts stored
sessions for the current agent command + project. Clicking it makes the
next Send resume the most recent one via `session/load` (agents without
`loadSession` fall back to a fresh session with a notice). Sessions are
recorded after each successful turn, only when the agent can resume
them. Replayed history renders as one text stream (user and agent
chunks are not yet distinguished). The count refreshes when the panel
re-renders (config/mode change), not live from disk.
