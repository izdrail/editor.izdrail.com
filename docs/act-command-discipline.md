# Act: file edits stay in the reviewed proposal (item 211, host QA follow-up)

A capable model (Qwen3.5-9B) asked run_command for
`sed -i 's/one/1/g' a.txt` instead of ending with the changes JSON -
an edit that bypasses the review surface entirely. Two defenses:

- The Act system contract now says it plainly (both with and without
  MCP tools): run_command is only for commands that do not change
  files (ls, tests, builds, git status); never create, edit, move or
  delete files with it; every file change goes in the changes JSON,
  the only way Act can change files.
- The approval card shows an advisory when a command looks like it
  writes files: "This command looks like it changes files - those
  edits bypass owner review." The check (agent_loop::
  command_writes_files) is a documented heuristic: sed -i /
  --in-place, output redirects (>, >>, >file, 2>file, &>file), and
  tee/dd/mv/rm/cp/mkdir/rmdir/touch/truncate/install/patch/ln. It
  blocks nothing; the owner's click stays the gate.

Coverage: command_writes_files is unit-tested across mutating shapes
(in-place sed, redirect variants, tee, rm, path-qualified mv) and
read-only shapes (ls, cargo test, quoted arrows, sed without -i, git
status).
