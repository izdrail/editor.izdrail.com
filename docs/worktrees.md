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
