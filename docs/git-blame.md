# Git: blame current file (item 178)

Command palette: **Git: Blame Current File** (`git_blame`) runs
`git blame --line-porcelain` on the active file and opens the result
in a scratch editor, rendered as aligned columns:

```
abc12300  Alice Example      2025-06-15  | let answer = 42;
--------  Not Committed Yet  2025-06-15  | let better = 43;
```

- Parsing is a pure state machine over the porcelain format
  (`parse_blame_porcelain`): --line-porcelain repeats every key per
  line group, so no cross-group bookkeeping is needed.
- Dates are epoch seconds rendered as UTC `YYYY-MM-DD` via a small
  civil-from-days implementation (`format_epoch_utc`) - no new
  dependency. Uncommitted lines (all-zero SHA) show `--------`.
- Runs off the UI thread; failures (not a git repo, untracked file,
  empty file) toast clipped stderr. Same scratch-editor lane as Git
  History's `git show` view.

Coverage: golden tests for the date algorithm (epoch 0, +/-1 day, a
2025 timestamp), porcelain parsing including the uncommitted group,
exact column alignment, and a real-repo integration test.

Follow-up: gutter annotations reusing this parser, and opening the
blamed commit in the history view on click.
