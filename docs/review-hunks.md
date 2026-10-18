# Hunk groundwork for per-hunk Act review (item 186)

`agent_changes::hunks(base, proposed)` splits a change into
non-overlapping line regions: common prefix/suffix trim, then a line
LCS over the middle (bounded at 1M DP cells; larger middles fall back
to the single-region behavior the review view already shows).
`merge_hunks(base, hunks, accepted)` rebuilds a document from a
per-hunk accept mask - full accept reproduces the proposal exactly,
full reject reproduces the base.

This lands the tested core for per-hunk review. Wiring it into the
digest-pinned stage() transaction and the review panel's per-hunk
toggles is the next item; the proposal format and stage semantics are
unchanged here.

Coverage: separate-region splitting, insertions/deletions, no-op,
partial/full accept roundtrips, and missing-final-newline
preservation all have behavior tests in agent_changes.
