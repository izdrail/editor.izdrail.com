# Plan artifacts index (item 195, Antigravity parity: artifacts view, first slice)

The palette command "Plans: Index and Open Artifacts"
(open_plan_artifacts) renders the workspace's `.lapce/plans` directory
as `.lapce/plans/index.md` - newest first, up to 50 entries, each with
the plan's local time, its first heading (or "(untitled plan)"), and
its relative path - and opens the index in an editor tab. This closes
the discoverability half of the artifacts gap: Plan-mode answers were
already written to `.lapce/plans/plan-<epoch>.md` and opened on
creation, but there was no way to find older ones short of the file
tree. The dedicated chat-panel artifacts section (browsable in-panel,
like the Agents list) is the follow-up slice.

Coverage: plan_index is behavior-tested with a real tempdir (newest
first ordering, heading extraction and the untitled fallback,
non-plan files and bad stamps skipped, empty-directory text).
