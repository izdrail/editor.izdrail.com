# Git: Pull (item 166)

Command palette: **Git: Pull (fast-forward only)** (`git_pull`) runs
`git pull --ff-only` for the current workspace off the UI thread and
reports through the toast messages, exactly like Git: Push (item 164).

Fast-forward only is deliberate for a palette action: it never creates
a surprise merge commit, and a diverged branch fails cleanly with
guidance (`git pull --rebase` or merge in a terminal, then pull
again). `GIT_TERMINAL_PROMPT=0` applies here too: credential prompts
fail fast instead of hanging a hidden thread.

Detail text comes from stdout when git speaks there (pull reports
"Already up to date." on stdout), falling back to stderr, clipped.
