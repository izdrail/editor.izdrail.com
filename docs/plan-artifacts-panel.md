# Chat panel Plans section (item 196, Antigravity parity: artifacts view, second slice)

The chat panel now shows a "Plans" section under the Agents list: the
workspace's three newest `.lapce/plans/plan-<epoch>.md` artifacts as
clickable rows (each plan's first heading, or "(untitled plan)"). A row
opens the plan as a normal editor buffer, so review is the same edit-
the-file flow introduced with plan artifacts. The section hides itself
when the workspace has no plans; the full 50-entry index is one
palette command away ("Plans: Index and Open Artifacts", item 195).

The section refreshes on panel build and whenever a plan artifact is
written: ChatData carries a `plans_version` signal that
write_plan_artifact bumps after a successful write, and the section's
effect re-reads the directory when it changes. Each row keeps the root
it was listed from, so clicking still opens the right file after a
workspace switch. The directory scan itself moved into a shared
`agent_loop::plan_entries` helper - the markdown index (item 195) now
formats from it, and the panel lists from it, so ordering, title
extraction and the 50-entry cap can never drift apart.

Coverage: plan_entries is behavior-tested with a real tempdir
(newest-first ordering, file name and title rows, the untitled
fallback, non-plan files skipped, missing directory yields an empty
list). Item 204 extended the shared lister to also accept
`walkthrough-<epoch>.md` artifacts, so applied-change walkthroughs
appear in this section and the index alongside plans; the existing plan_index test keeps the markdown output pinned
through the refactor.
