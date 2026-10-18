# Git: create branch (item 170)

The SCM references palette (click the branch name in the status bar,
or **Source Control: Checkout**) now offers **Create new branch
'<name>'** whenever the typed text is not an existing branch or tag.
Selecting it creates the branch at HEAD and switches to it, mirroring
VS Code's `git checkout -b` flow. Because the new branch points at the
same commit, creation never refuses over uncommitted changes;
duplicate names and invalid refnames surface the git error instead of
overwriting.

The **Source Control: Create Branch** command
(`source_control.create_branch`) is also listed in the command
palette; without a name argument it opens the SCM references palette
where the name is typed.
