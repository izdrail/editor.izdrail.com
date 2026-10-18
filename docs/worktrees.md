# Git worktrees

**Git: New Worktree** (command palette) creates a git worktree of the
current workspace on a new timestamped branch (`worktree/<unix>`) from
HEAD, in a sibling directory (`<folder>-worktree-<unix>`), and opens it
as a new workspace tab. The agent lanes (scheduled prompts, ACP
sessions) are per project, so a worktree tab gets its own schedules and
agent sessions - agent work can happen there without dirtying your main
checkout.

Honest limits: the branch name is timestamped, not chosen (rename it in
git afterwards if it becomes permanent); there is no worktree list or
removal command yet - `git worktree list/remove` in a terminal covers
that. The command needs the workspace to be a git repository with at
least one commit; failures (not a repo, unborn HEAD, dirty state rules)
surface as an alert with the git error, clipped to 400 bytes. The git
call runs off the UI thread, so a slow checkout does not freeze the
window.

## Browsing worktrees (item 160)

Command palette: **Git: Worktrees** (`palette.worktrees`) lists every
worktree of the current workspace from `git worktree list --porcelain`,
labeled by branch (`(detached)` when there is none), the active one
marked `(current)` and preselected. Enter opens the chosen worktree as
a new workspace tab - the same tab shape item 158 opens, with its own
chat sessions and scheduled prompts. Choosing the current worktree is
a no-op. When the workspace is not a git repository (or git fails) the
list shows a single explanatory no-op row instead of going blank.

The list is read inline when the palette kind opens: it is one fast
local `git worktree list`, not a checkout, so it does not get the
spawned-thread treatment item 158 needed. Porcelain paths with unusual
bytes arrive C-style quoted (core.quotePath) and are unquoted by
`worktree::parse_worktree_list`, which is fuzzed with quoted non-ASCII
paths, unknown attributes and malformed records.

Still not here: multi-folder projects and per-worktree permission scopes.

## Removing worktrees (item 161)

Command palette: **Git: Remove Worktree** (`palette.remove_worktree`)
lists every worktree except the one this window has open (git cannot
remove the worktree you stand in). Enter does not delete anything: it
opens a confirm dialog naming the branch and path, and only the
dialog's Remove button runs `git worktree remove` on a spawned thread.

Two independent guards: the confirm dialog (a palette misclick cannot
delete a directory) and git's own refusal when the worktree has
uncommitted or untracked changes - there is deliberately no --force.
The branch is always kept; the success dialog says how to delete it
with `git branch -d`. A refusal surfaces git's stderr, clipped.

Known limitation: a window tab already open on the removed worktree is
left pointing at a deleted directory; close it yourself.

## Dirty badges (item 168)

Both worktree palettes mark rows `(dirty)` when
`git status --porcelain` reports anything (modified, staged or
untracked - the same states that make `git worktree remove` refuse).
Checks run live when the palette opens, bounded to the first
MAX_DIRTY_CHECKS (8) rows so a pathological worktree list cannot
stall the palette; a worktree that cannot be checked counts as clean
(a badge is a hint, not a gate). In the remove palette the badge is
the useful warning: dirty rows are the ones git will refuse.
