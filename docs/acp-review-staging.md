# ACP file writes land in review staging (item 182)

The ACP session now advertises `fs/writeTextFile: true` and stages
agent writes for owner review instead of refusing them:

- `fs/write_text_file` requests are confined to existing files under
  the canonical workspace root (same confinement as reads), capped at
  2 MiB and 32 files per turn. Last write per path wins. The agent
  gets a success response - the write IS accepted, into the review
  lane; it is NOT on disk and NOT in any buffer yet, and the turn's
  step cards say so ("staged a write to ... for owner review").
- When the turn ends, the staged set is drained, converted to
  workspace-relative paths, and handed to the unchanged owner-review
  lane (`ReviewData::prepare`), which re-validates autosave-off,
  open-buffer, line-ending and size rules before showing the
  before/after. Only the owner's Apply touches buffers.
- Known v1 limits (surfaced as refusals with reasons): writes to
  files that do not exist yet, and writes to files not open in the
  editor (the review lane is buffer-backed).

Coverage: session tests (staging + last-write-wins + drain, disk
untouched; refusal of outside-workspace, missing and oversized
writes; capability advertised).
