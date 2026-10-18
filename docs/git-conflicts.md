# Git: conflicted files in the change list (item 174)

Files with unresolved merge conflicts were previously dropped from the
source control change list entirely (`Delta::Conflicted` fell through
to `None`), so a conflicted pull made the affected files vanish from
the panel. They are now listed with the warning icon in the error
color, sorted with the rest of the changes.

Two deliberate refusals keep the conflict state honest:

- **Commit refuses conflicted paths** proxy-side: `git add` on a
  conflicted entry would mark it resolved with the conflict markers
  included, so the commit errors with "resolve them in the editor
  first" instead.
- **Discard Changes on a conflicted row does nothing** (the proxy
  checkout refuses against a conflicted index): abandoning merge
  state needs an explicit `git merge --abort`, not a file restore.

Clicking a conflicted row opens the usual diff against HEAD, which
shows the conflict markers in place. Follow-ups: an editor-level
accept-current/incoming/both surface and a merge-abort command.

## Resolving hunks (item 175)

Command palette, operating on the hunk enclosing the cursor in the
active editor: **Git: Accept Current Change** (`git_accept_current`),
**Git: Accept Incoming Change** (`git_accept_incoming`), **Git:
Accept Both Changes** (`git_accept_both`). The block detection is a
pure function (`conflict::resolve_conflict_at_line`, unit-tested for
each side, enclosing-block selection, marker-line cursors, empty
sides, and unterminated blocks); the edit goes through the standard
LSP workspace-edit lane, so it applies and undoes like a
rename/refactor. diff3-style conflicts (with `||||||| base` sections)
are detected and refused with a toast until supported.

**Git: Merge Abort** (`git_merge_abort`) runs `git merge --abort` off
the UI thread with the usual toast feedback - the explicit escape
hatch the Discard refusal references.

Dedup fix: a conflicted file no longer also appears in Staged Changes.
During a merge the index holds the file's stage entries, so the
HEAD->index diff reported it as a staged modification next to the
workdir Conflicted row. git_diff_new now drops conflicted paths from
the staged list - the file lists once, under Changes with the warning
icon and commit refusal. Covered by a real-merge test
(conflicted_file_lists_once_not_in_staged: branch, diverge, merge,
assert Changes has the Conflicted row and Staged does not).
