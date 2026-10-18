# Agent sessions

The AI Chat panel keeps a per-project list of past ACP agent sessions
(the external agents configured under `acp_command`). Click **Agents (N)**
in the chat mode row to open the list: each row shows the session title,
the agent that owns it, and its age. Clicking a row switches the panel to
ACP mode and resumes exactly that session on your next message.

**Resume (N)** still picks up the most recent session for the current
agent and project; the Agents list is for picking an older one or a
different agent's session. New chat and the mode buttons clear a picked
session again.

Sessions are stored in `.lapce/acp/sessions.json` inside the project (at
most 64, most recent first), so the list is per project and survives
restarts. Honest limits: this is a session browser for one window, not
multi-window mission control - no cross-workspace view and no parallel
agents yet, and resuming a session replays it in the current panel rather
than running alongside it.

## Deleting sessions (item 163)

Each row in the Agents list has a **Delete** button. It removes the
stored resume record from `.lapce/acp/sessions.json` and updates the
list in place; if the deleted session was the composer's selected
resume target, that selection clears too. There is no confirm: the
record is a resumability pointer, not your data - the agent's own
session lives in the agent. Deleting a record never stops a running
agent turn. A missing id is a no-op.
