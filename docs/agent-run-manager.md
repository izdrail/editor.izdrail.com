# Runs section: window-wide live-run manager (item 205, Antigravity parity: parallel agents, first slice)

Runs in different workspace window tabs already execute in parallel -
each tab owns its chat state and worker - but there was no way to see
or stop them from one place. The chat panel's new "Running" section is
that manager surface: every agent run in the window, newest first, as
"Mode · HH:MM - first line of the prompt", with a Stop button wired to
the run's own cancellation flag (the same flag the panel's Stop button
sets, so a registry Stop behaves exactly like an in-panel Stop,
including Act's pending approval cards and ACP's subprocess kill).

The registry is a small `agent_runs::RunRegistry` on the window-level
`WindowCommonData`, shared by every tab. Each chat send path (chat,
Ask/Plan/Act, ACP) registers its run right after it goes busy, and
every termination path routes through a new `ChatData::end_active_run`
that finishes the registry entry and clears busy together, so the
section cannot list a finished run. Removal sites are auditable: the
patch replaces every `busy.set(false)` in chat_view.rs.

Known limit: a workspace tab closed mid-run leaves its entry until the
window closes (its Stop flag has no listener left). Per-session
background runs inside one panel - the rest of the parallel-agents
parity - build on this registry next.

Coverage: RunRegistry is unit-tested (newest-first ordering, id
uniqueness, finish removes and is idempotent, the 32-entry cap is
implicit in start, stop flag is the same Arc the run observes);
run_title is unit-tested (first line only, 40-char cap with ellipsis,
empty prompt fallback).
