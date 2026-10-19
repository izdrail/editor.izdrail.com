# Making Act work with small local models (items 225-226)

Stefan's report: Act with qwen2.5-coder:1.5b answered an unrelated
node tutorial and the run died with "the answer did not parse:
expected value at line 1 column 1". Three root causes, three fixes:

1. The agent lane never sent the editor context. The "On Send: File:
   X | N bytes" chip describes the Chat lane; Ask/Plan/Act prompts
   carried only the typed text and @-mentions, so the model never saw
   the file. send_agent now appends the same <editor_context>
   envelope the Chat lane uses (item 225).

2. run() rejected prompts over 12000 bytes while @-mentions budget
   256 KiB and the editor context adds up to 12 KiB. The cap is now
   280000 so attaching a real file cannot fail validation (item 225).

3. The proposal parser demanded the whole answer be one strict JSON
   object. extract_proposal_json now finds the proposal in fenced
   blocks, surrounding prose, or among other JSON objects
   (string/escape-aware balanced scan), and - as a first-class
   no-JSON path for tiny models - maps exactly one fenced block to a
   full-file replacement for the open file (item 226). The same
   extraction backs the paste-based Prepare review lane.

When an Act final answer still has no proposal, the loop feeds the
parse error back to the model with the schema (and the fenced-block
alternative when a file is open) and retries once. If that also
fails, the error says so and suggests a larger tool-capable model or
narrowing the task to one file.

Exception: a run that executed at least one tool call may end with a
prose REPORT instead of a proposal - the owner can legitimately ask
Act to inspect and report ("run ls -la and tell me the output; do not
propose edits"). Only runs where the model never engaged the tools
are gated; tool-using runs pass their final answer through, and the
review lane's extraction still guards anything handed to Prepare.
