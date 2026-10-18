# Git: Push (item 164)

Command palette: **Git: Push** (`git_push`) runs `git push` for the
current workspace off the UI thread and reports through the toast
messages (INFO on success, ERROR on failure, WARNING when no folder is
open).

`GIT_TERMINAL_PROMPT=0` is set on purpose: a credential prompt on a
hidden thread would hang forever, so git instead fails fast with a
clear message. SSH agents and credential helpers work unchanged.
Failures surface git's stderr (clipped) with the two common fixes:
create an upstream with `git push -u origin <branch>`, and set up a
credential helper or SSH agent.

Honest limits: this pushes the current branch to its configured
upstream only - no remote/branch picker, no force options, no progress
bar. A stalled network can still leave the thread waiting; the editor
stays responsive and the next push just starts another thread.
