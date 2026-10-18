# Git: stash (item 171)

Command palette: **Git: Stash** (`git_stash`), **Git: Stash (Include
Untracked)** (`git_stash_include_untracked`, adds `-u`), and **Git:
Stash Pop** (`git_stash_pop`). Each runs `git stash push` / `git stash
pop` off the UI thread and reports through the toast messages, with
the stdout/stderr split owned by `stash_result_message` /
`stash_pop_result_message` in `worktree.rs` (stash reports outcomes
like "Saved working directory..." and "No local changes to save" on
stdout, failures on stderr).

A conflicting pop keeps the stash entry - the error toast says so.
`GIT_TERMINAL_PROMPT=0` for the same reason as push/pull: a hidden
thread must never block on a credential prompt.

This also makes the worktree-refusal guidance ("Commit, stash or
clean them in that worktree first") actionable from the palette. A
visible stash list in the source control panel is a deliberate
follow-up; `git stash list` in a terminal covers inspection today.
