# Staged section render fix (bugfix on item 174's staged/unstaged split)

Report: a file staged outside the app (plain `git add`, or any staging
that leaves the workdir clean) never appeared in the Source Control
panel - "Staged Changes" rendered empty while "Changes" listed only
the unstaged files, even after a fresh start.

The data path was intact end to end: the proxy computes the staged
list from HEAD-tree -> index (dispatch.rs, covered by
staged_and_unstaged_are_separate / file_staged_then_modified_again),
DiffInfo carries it, and the window-tab handler stores it on the
panel model. The break was layout: the staged section's body had no
definite height anywhere - the inner scroll is absolutely positioned
(so it contributes no intrinsic size) and, unlike the Changes section
(size_pct(100,100)), the staged section and its view only set width.
The body computed to zero height and every staged row was invisible.

Fix: the staged view now follows the chat panel's Agents-list pattern
- a content-sized scroll capped at max_height(180) - so the section
shows its rows and scrolls past the cap, while Changes keeps the
remaining space. The handler's DiffInfo -> panel-list mapping also
moved into a pure `merge_diff_info` (source_control.rs) so the
section-membership contract is unit-testable: staged files list under
Staged and only there, workdir checkbox state survives refreshes, and
unstaging moves a file back to Changes (new rows default to checked).
Staging from inside the app (Stage/Unstage context menus, hunk
staging) flows through the same DiffInfo refresh, so it lands in the
same merge.
