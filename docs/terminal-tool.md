# Act mode: run_command tool with per-command owner approval (item 181)

Act runs can now execute shell commands, one at a time, each behind
an explicit owner decision:

- The agent loop registers `run_command` **only in Act mode**
  (Ask/Plan keep their read-only contract; a run_command proposal
  there fails validation). The loop's tool-arg validation now keys
  off a registry (`tool_arg_key`) instead of if/else chains.
- When the model proposes a command, the worker sends an approval
  request to the chat panel and blocks. The panel shows an approval
  card above the review section: the exact command, **Allow once**,
  **Deny**. Nothing is remembered - every command asks again, a
  Deny (or a dropped card) goes back to the model as the tool result,
  and the card clears when the run finishes, errors, or is cancelled.
  An unanswered approval times out after 5 minutes.
- Approved commands run via `agent_exec::run_shell_command`:
  `sh -c` in the project root, stdin closed, 30s wall-clock cap,
  killed on cancel, combined stdout+stderr capped at 64 KiB on a
  char boundary, non-zero exits returned as errors with the output.

Coverage: agent_exec tests (echo roundtrip, failing exit, timeout
kill, output cap, empty/oversized rejection); loop tests (Act
run_command executes through the host executor; Ask-mode run_command
proposals are rejected without executing).

Update (r3): the runner spawns `sh -c` in its own process group and
kills the whole group on every exit path (timeout, cancel, success),
so pipeline grandchildren can neither hold the output pipes past the
wall-clock limit nor survive the command. Reader threads drain both
pipes to EOF, so large output is clipped, never deadlocked.
