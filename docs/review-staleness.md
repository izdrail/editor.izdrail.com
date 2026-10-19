# Review staleness surfacing (item 207, AI parity: digest-pinned context, host-side slice)

An Act run reads files, then proposes full-file replacements derived
from what it read. If the owner edits one of those files while the run
is in flight, the proposal is derived from older content - and because
review diffs against the live buffer, the owner's own edits appear in
the diff as removals. Visible, but easy to miss.

The host now pins it with digests instead of trusting anyone to
notice: the send path records the sha256 of every file content the
agent actually received from read_file (what the model saw, not what
is on disk, capped at 64 entries), and ReviewData::prepare_with_reads
compares each proposed change's live base against the recorded digest.
Mismatches land in a new `stale` list: the review surface shows
"Changed since the agent read them: <paths>" and the notice warns that
the proposal may clobber those edits. Nothing is blocked - the owner
still decides, now with the clobber made explicit. Discard clears the
list; a prepare without read digests (the paste lane) sets none.

This is the no-contract-change slice of the digest-pinned context
machinery: agent_act_contract's full Context capture/review_candidate
flow (model echoes base digests inside the proposal) stays available
for a future async/multi-turn proposal flow, as documented when Act v1
shipped.

Coverage: stale_against_reads is unit-tested (flags only paths whose
live base moved since the recorded read, ignores paths the agent never
read, fresh reads produce no staleness).
