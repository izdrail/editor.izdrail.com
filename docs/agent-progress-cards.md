# Agent progress cards (step cards)

Ask/Plan/Act runs show their progress as a collapsible card inside the
assistant turn, Antigravity-style: a "Progress updates (n)" header with
a fold chevron, then one row per loop event.

Rows come from the read-only agent loop's events (`agent_loop::Event`):

| Loop event | Row |
| --- | --- |
| `Step(n)` | "Step n" - "Reading and reasoning - no files are changed." |
| `ProposedCalls(calls)` | one row per call: tool name + target (path or search literal, clipped to 120 chars) |
| `ToolResult { id, error }` | tool id + "read completed" / "denied or failed" (error rows in the error color) |

The rows live on the turn (`Turn.steps`), are session-only (never
persisted to chat history), and are capped at 128 per turn (the loop's
own limits keep real runs far below that). Plain chat, ACP and
history-loaded turns have no steps and render no card.

Files-edited chips like Antigravity's are not applicable yet: the agent
lane is read-only (read_file, search). When the loop gains edit tools,
their proposals should land as rows here plus a files chip row.
