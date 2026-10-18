# Git: history palette (item 172)

Command palette: **Git: Show History** (`palette.git_log`) lists the
last 200 commits (`<short sha> <subject> (<author>, <relative>)`,
parsed from `git log` with a U+001F field separator so subjects are
never mangled). Selecting an entry runs `git show --format=fuller
<sha>` off the UI thread and opens the result in a scratch editor with
diff syntax highlighting; output is capped at 512 KB
(`clip_show_output`) so a mass-rename commit cannot swamp a buffer.

The `git log` fetch is synchronous at palette open - bounded and
normally tens of milliseconds, but on a cold cache it blocks the
palette briefly. An async fill (the references-lane signal pattern)
is the known follow-up. Scratch tabs are the standard untitled-N lane;
named scratch sources are another follow-up.
