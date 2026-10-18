# Git: ahead/behind in the status bar (item 169)

The status bar branch label now shows how far the checked-out branch is
from its configured upstream: `main ↑2` when two local commits are
unpushed, `main ↓1` when the upstream has one commit you have not
pulled, `main ↑2 ↓1` when the histories have diverged. Zero counts are
hidden, and the dirty `*` marker still comes last.

The counts ride the existing `DiffInfo` refresh (same poll as the
change list), computed by the proxy with libgit2
`git_graph_ahead_behind` against the branch's configured upstream -
no fetch is performed, so the numbers reflect your last fetch, exactly
like `git status` does.

Branches without an upstream, detached HEADs, and repos mid-init all
report `(0, 0)` and render as plain branch names; no error surfaces.
The same data is available to the future pull/push indicators
(items 163/164).
