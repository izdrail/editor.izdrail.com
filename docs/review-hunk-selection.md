# Per-hunk accept in Act review (item 187)

The review panel now renders each change as its computed hunks with a
per-hunk include/skip checkbox (default: all included). "Apply the
accepted hunks" sends the owner's mask with the proposal.

The digest-pinned transaction keeps its guarantee: `stage()` is
unchanged; the new `stage_with_selection(review, current, masks)`
still fails first when the displayed proposal's digest does not match,
then validates every mask against hunks recomputed from the pinned
base/proposed texts (a stale or wrong-shaped mask never stages), and
merges only accepted hunks. A change with no accepted hunk is left
untouched; rejecting everything is an explicit error, never a silent
no-op. Checkpoints record the merged text, so Restore still reverts
exactly what was applied. Checkpoint rows keep the plain whole-region
diff display (no toggles - restore is all-or-nothing).

Coverage: partial accept, all-accept == full stage, all-reject error,
wrong-length and empty masks, and digest tampering are tested in
agent_changes. The checkbox row rendering is visual/host-verified.
