# ACP chat panel wiring (item 135)

The chat panel gains an ACP mode: the `ACP` button next to Chat/Ask/Plan
routes Send to a configured external agent subprocess over the Agent
Client Protocol, instead of the built-in chat or the Ask/Plan loop.

## Setup

Settings > AI Chat > `acp-command`: the command that starts an ACP
agent, for example `opencode acp` (native) or a Claude Code/Codex
adapter binary. Empty (the default) disables ACP mode with a hint.
Whitespace splitting only; keep the binary path space-free.

## Behavior

- One Send spawns a fresh agent process rooted at the current project
  folder (canonicalized). Session continuity across sends (the spec's
  `session/load`) is a later milestone.
- The reply body shows setup status lines, then the streamed agent
  message chunks, plan entries, tool call titles, fs-read notices and
  usage updates, separated by blank lines.
- Agents can read project files through the session controller
  (workspace-confined, 1 MiB cap); writes are refused until the review
  UI owns them. The agent's own tools still run under the agent's own
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
