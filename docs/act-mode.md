# Act mode v1: reviewed full-file proposals (item 179)

The chat panel's mode picker now offers **Act** next to Ask and Plan.
Act runs the same bounded, read-only agent loop (read_file + search
tools only, same step/call/time/size limits) with one difference: its
contract requires the final answer to be ONLY a JSON object of
full-file replacements:

```json
{"changes":[{"path":"relative/project/path","replacement":"complete new file content"}]}
```

The reply is handed to the existing owner-review lane
(`ReviewData::prepare`), unchanged: it re-validates that autosave is
disabled, the platform is Linux, every path is an existing open
buffer under the project root, line endings are normalized, and the
1-32 file / 2 MiB limits hold. Only then does the review section show
the before/after for every file. **Nothing is applied by the agent,
ever** - the owner clicks Apply (writes to open buffers only, with a
checkpoint for restore) or Rejects. Failed validation keeps the raw
reply visible in the chat and shows the reason in the error line.

This is deliberately v1: no hunk-level edits, no new-file creation,
no disk writes, no ACP writes (still refused). The
`agent_act_contract` digest-pinned Context machinery stays available
for a future async/multi-turn proposal flow; v1 binds freshness by
capturing base state at review time instead.

Coverage: the loop's Act gate test became an Act happy-path test
(read, then answer with proposal JSON; system prompt contains the Act
contract); review-lane validation and application remain covered by
the existing agent_review_ui / agent_changes tests.
