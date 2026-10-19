# Walkthrough artifacts (item 204, Antigravity parity: walkthrough after a completed change)

Applying a reviewed proposal now records what landed as
`.lapce/plans/walkthrough-<epoch>.md`: the run id, the applied
proposal's digest, and one row per changed file with the number of
accepted hunks (taken from the owner's per-hunk toggles when they
reviewed hunk by hunk, so the walkthrough reflects what actually
landed, not what was proposed). The notice line names the new file,
and the chat panel's Plans section refreshes immediately because the
apply bumps the shared `plans_version` signal.

The walkthrough is written by the single digest-checked apply path in
`agent_review_ui`, so both Act proposals and pasted change-sets get
one. The content comes from a new pure `agent_loop::walkthrough_artifact`
builder, and the shared `agent_loop::plan_entries` lister now accepts
both `plan-<epoch>.md` and `walkthrough-<epoch>.md`, so walkthroughs
appear alongside plans in the panel section and in the "Plans: Index
and Open Artifacts" index. The artifact never fails the apply itself:
the buffers and checkpoint journal have already landed, so a write
error is appended to the notice instead. With no workspace root open,
no walkthrough is written.

Coverage: walkthrough_artifact is unit-tested (run id, digest, paths,
singular/plural hunk counts, the empty-apply fallback, trailing
newline); plan_entries and plan_index tests now pin walkthrough
names in the ordering and reject bad walkthrough stamps.
