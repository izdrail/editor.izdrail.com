# Per-session background runs (spec, item 235 / parity 7)

Antigravity parity: an agent run belongs to its SESSION, not to the
panel that happened to start it. Closing or switching away from a
chat tab must not strand the run, lose its result, or leave a
"Running" row that never ends.

## Current architecture and its three failure modes

- send_agent spawns a worker thread per run. The worker emits
  AgentUpdate values over an mpsc channel to the panel's effect;
  the panel renders them and, on Done/Error, calls
  ChatData::end_active_run -> RunRegistry::finish(id).
- The RunRegistry is window-wide (agent_runs.rs); every entry has a
  cancellation flag the panel's Stop and the Runs section share.

Failure modes (all observed in the code, agent_runs.rs header names
the first):

1. STRANDED ENTRY: if the panel (its rx) is dropped mid-run - tab
   closed - every tx.send fails silently and nobody calls finish().
   The Runs section lists the run as live until the window closes.
2. LOST RESULT: the final reply/proposal travels only via the
   channel. With no listener the completed work vanishes; the
   session file never records the turn.
3. NO REATTACH: a panel opened (or reopened) for a workspace with a
   live run has no way to see the run's progress or outcome; the
   run is only visible as a row in the Runs section.

Restart across app quit is OUT OF SCOPE: Session::recover already
marks a mid-run session Interrupted on the next window.

## Design

The run's truth moves into the registry entry; panels become
listeners that attach and detach freely.

RunEntry gains:

- workspace: PathBuf - which workspace (== session) the run belongs
  to; how a newly opened panel finds its run.
- journal: Arc<Mutex<RunJournal>> - the bounded, thread-safe record
  of what the run did: step rows (title, detail, error, capped at
  200 entries / 256 KiB) and, at the end, the outcome (Done(reply)
  / Error(text) / Cancelled). The worker writes EVERY AgentUpdate
  to the journal first, then attempts the panel channel.
- listeners: Arc<AtomicUsize> - panels holding a live subscription.

Worker lifecycle (slice 1, as built): the worker owns the END of
its entry, not the registry itself - floem signals stay on the UI
thread everywhere in this codebase (every worker reports over
channels or the proxy; no signal crosses a thread boundary), so
RunRegistry::finish_guard(id) hands the worker a FinishGuard
holding only the entry's plain Arc<AtomicBool>. The guard's Drop
marks the entry worker-done on every exit path - normal, error,
cancel, even panic - and the existing one-second runs tick sweeps
flagged entries. The panel's end_active_run keeps finishing
through the normal path; finish() is idempotent (retain on an
unknown id is a no-op), so whichever side ends the run first
wins and the other's cleanup is a harmless no-op. The local-model
chat lane has no spawn of its own: its guard lives inside the AI
worker's completion callback, dropped when the worker drops it.

Detached completion (slice 2): when the run ends with
listeners == 0, the worker appends the completed turn (the owner's
prompt, the reply or the error text, and the step count) to the
workspace session file via chat_session::save, honouring the
save_history setting that run was started with. Session::validate
caps (200 turns / 2 MiB) already bound the file.

Reattach (slice 3): on chat panel init, the panel asks the
registry for a live entry matching its workspace root. On a match
it bumps listeners, replays the journal into the in-flight turn's
step rows, and drains new journal entries on the existing
one-second runs_tick (each panel remembers its drained index; the
worker only appends, so draining is index arithmetic). On Done/
Error/Cancelled the panel renders the outcome, bumps down
listeners, and runs its normal end-of-run bookkeeping. A panel
dropped mid-run simply stops draining; the journal remains correct
for the next listener.

UI (slice 4, last): the Runs section labels each entry with its
workspace folder name when more than one workspace has live runs.

## Slices

1. Worker-owned finish (drop guard) + idempotent finish + tests
   (finish exactly once on Done, Error, Cancelled, and a panicking
   worker; no lingering entry when no panel ever listens).
2. RunJournal on RunEntry + worker dual-emission + detached
   completion persist honouring save_history + tests (journal caps;
   persisted turn shape; no persist when a listener was attached).
3. Reattach: workspace key matching, journal replay + tick-driven
   drain, listener counting + tests (replay from a mid-run journal;
   drain sees only new entries; outcome ends the drain).
4. Runs-section workspace labels (one-liner UI patch, render-only).

## Risks and rules

- Exactly-once finish: the drop guard is the only finisher; the
  panel path becomes a no-op when the worker already finished.
- Persist ownership: only the worker writes session files for
  detached runs; an attached panel persists as today. The listener
  count decides - never both.
- Journal bounds: steps cap at 200 entries and 256 KiB total, same
  order as the panel's own caps; overflow drops oldest with a
  marker row, never blocks the run.
- No new wake/timer: the drain rides the existing runs_tick.
