# Agent-edit attribution in source control (item 212, parity row 22)

Parity gap: the VCS panel could not tell agent-made changes from the
owner's own edits. The Changes list now shows a small "agent" badge on
any file an applied agent run recorded in its walkthrough artifact.

Source of truth: applying a reviewed proposal already writes
`.lapce/plans/walkthrough-<stamp>.md` listing the changed files
(item 204). The panel rescans that directory when the diff list
changes and badges matching relative paths. Attribution is advisory:
it blocks nothing, missing or unreadable artifacts are skipped, and
the badge only means "a completed agent run recorded changes here" -
the owner can still have edited the file afterwards.

Coverage: agent_touched_files is unit-tested against two artifacts
(dedupes across runs), a non-walkthrough decoy file, a no-changes
artifact, and a workspace with no plans directory.

Out of scope for this slice: staged-list badges (the staged section
can reuse the same memo), hunk-level attribution, and clearing badges
on commit (the file leaves the diff list on commit anyway).
