# Git: staged changes section + per-file stage/unstage (item 176)

The source control panel now splits the change list into two
sections: **Staged Changes** (index vs HEAD) above **Changes**
(workdir vs index). Previously the proxy merged both delta lists into
one, so staging a file did not visibly change the panel.

- Right-click a row under **Changes** for **Stage** (`git add --`),
  right-click a staged row for **Unstage** (`git reset -q --`). Both
  run off the UI thread (`worktree::stage_paths`), are silent on
  success - the existing .git watcher refreshes the panel - and toast
  clipped stderr on failure. `GIT_TERMINAL_PROMPT=0` keeps the
  background thread from ever blocking on a credential prompt.
- Staged rows keep the same layout minus the commit checkbox; the
  checkbox set still drives only the commit message list, and the
  commit itself goes through the real index, so staging semantics are
  unchanged whether you stage via the panel or a terminal.
- The proxy computes the two lists from the same delta pipeline
  (`deltas_to_file_diffs`, extracted unchanged from the old single
  list), so rename pairing and conflict markers behave identically in
  both sections. On an unborn branch (fresh repo without commits) the
  staged diff runs against the empty tree, listing every added file.

Coverage: proxy tests stage and re-modify files in a scratch repo and
assert the two `DiffInfo` lists stay separate
(`git_staged_diff_tests`); `worktree::stage_paths` round-trips a real
`git add` / `git reset` against a temp repo.

Follow-up: hunk-level staging (stage the hunk at the cursor) via
`git apply --cached`.
