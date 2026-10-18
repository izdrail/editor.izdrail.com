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
