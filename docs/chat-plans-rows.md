# Chat panel: Plans rows (item 305)

The Plans list in the Chat panel used to render its rows side by side and show only each artifact's first heading, so three walkthroughs read "Walkthrough Walkthrough Walkthrough". Rows are now stacked one per line, each as `Kind · title · time` (the title is dropped when it only repeats the kind), clipped with an ellipsis instead of overflowing. Clicking a row still opens the artifact as before.

Host capture: with at least two walkthrough files in `.lapce/plans`, open the Chat panel and check one row per line, distinct times, no overflow at a narrow panel width.
