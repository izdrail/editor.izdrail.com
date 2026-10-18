# Behavior-test pass for the shipped git + Act surface (item 180)

Real-behavior tests (subprocess against real temp repos / real proxy
calls) for everything shipped in items 170-179, replacing
verification-by-inspection where a runner exists:

- **stash** (`stash_push`/`stash_pop` extracted from the window tab
  into `worktree`, thin wrappers left behind): roundtrip of a tracked
  modification, untracked files with and without the flag, and the
  clean-tree "No local changes" report.
- **history**: `git_log_lines` against a two-commit repo (order, full
  SHAs, subject+author); `git_show` (also extracted to `worktree`)
  renders a real commit and errors on a bad SHA.
- **commit** (proxy): a modified file commits for real on born and
  unborn branches; a `FileDiff::Conflicted` entry fails the whole
  commit naming the conflict, with the HEAD commit untouched.
- **Act review**: the draft-JSON gate is now a pure total function
  (`parse_draft`) with tests for the accepted shape, empty/oversized
  batches, unknown fields (`approved:true` can never smuggle trust),
  missing `replacement`, and over-limit payloads.

Already covered before this pass, not duplicated: ahead/behind,
create-branch, staged-diff separation, hunk staging (incl.
integration), blame (incl. real repo), conflict resolution, proposal
digest/stage/checkpoint, diagnostics report fields/rotation,
walkthrough rendering.

Honest limits: focus rings, the composer overlay dropdowns and the
review panel's floem views have no headless test harness - those stay
screenshot-verified on the host.
