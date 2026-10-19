# Checkpoint dismissal and panel width fixes (item 215, host QA follow-up)

Two findings from the b408c57 render:

- "Discard proposal" and "Restore checkpoint" showed a grey bar over
  their lower half. Root cause: not a button style bug but a
  horizontal scrollbar thumb. The 64-char proposal digest is
  unbreakable text, so it overflowed the narrow panel's scroll
  viewport; the thumb painted at the viewport edge, on top of the
  card's buttons. Fix: the displayed digest is clipped to a 16-char
  prefix with an ellipsis (new short_digest helper - internal pinning
  still uses the full digest, apply_with recomputes it from the
  in-memory proposal and never parses the label), and every text
  label inside the change cards (path, hunk rows, unified diff,
  BEFORE/AFTER columns) now wraps at the panel width instead of
  overflowing.
- A checkpoint from an older task persisted across restart and
  stacked under the new, unrelated proposal. Decision: checkpoints
  stay persistent (the journal is the only recovery path for buffers
  that were never saved; auto-clearing on restart would destroy
  restore data), but the card gains a "Dismiss" action next to
  Restore: it deletes the journal file and drops the checkpoint from
  the panel without touching any buffer. The owner explicitly gives
  up the one-click restore for those buffers. A missing journal file
  (deleted externally) is tolerated on dismiss.

Coverage: short_digest is unit-tested (short input unchanged,
64-char hex clipped to 16 + ellipsis, non-ASCII input never panics
on a char boundary).
