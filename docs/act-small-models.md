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

## Path forms and checkpoint cards (item 227, host QA on 253cd71)

qwen3:4b's first Act step was read_file with the ABSOLUTE path copied
from the editor-context line, which ReadRoot rejects. Fixes:

- The editor-context line names the open file workspace-relative.
- read_file maps absolute paths under the workspace to relative
  (agent_read::workspace_relative); paths outside the root stay
  absolute and are still rejected.
- Both Act contract variants now say: an attached <editor_context>
  already holds the open file's full content - do not read_file it
  again - and always use workspace-relative paths.

Also: checkpoint before/after diffs start collapsed behind a "Show
before/after (N file(s))" toggle, so a stale checkpoint card no
longer pushes the review section down the panel; Restore and Dismiss
stay visible.

## Exact-edit proposals and ambiguity refusal (item 228, host QA on 571af35)

A qwen3:4b Act run for "change dirty to clean" produced a proposal
whose replacement deleted two of three lines. Two hardenings:

1. Extraction refuses ambiguity: when an answer holds several
   DIFFERENT proposal JSON objects, extraction errors (triggering the
   repair retry / a parse error) instead of silently picking the
   first. The same proposal repeated verbatim still extracts.

2. Small models get a simpler contract: an exact-edit form
   {"changes":[{"path":"...","old_text":"exact existing text",
   "new_text":"its replacement"}]} - old_text must occur exactly once
   in the current file; the host derives the full new content
   (agent_changes::apply_exact_edit) and the normal hunk review shows
   the minimal diff. No whole-file retyping, no JSON-escaping a whole
   file. The full-replacement form still works; mixing or partial
   forms are rejected. Both Act contract variants document the form.

## Empty-changes retry, worked example, deterministic fallback (item 229, host QA on 4406e2b)

A qwen3:4b run for "change the word dirty to clean" answered
{"changes":[]} - valid JSON that passed extraction and died in the
review lane as "Review requires 1-32 files". Four fixes:

1. An Act run whose extracted proposal carries a zero-length changes
   array now counts as a failed proposal: the one repair retry fires
   with a "You returned no changes" message that restates the schema
   (with the open file's path filled in) and re-includes the request
   and file content (clipped to 8_000 chars).

2. The Act contract ends with a one-shot worked example naming the
   actual open file (dirty -> clean), so small models can copy the
   shape instead of inventing one.

3. The double-failure error is now user-readable: "The model returned
   no usable edits. Try a larger model (for example qwen3:4b) in the
   model dropdown, or rephrase the request." (The old
   "pre-review validation" technical text can only appear in the
   paste-a-proposal lane, where it is appropriate.)

4. Deterministic fallback after the retry fails: when the request is
   literally "change X to Y" / "replace X with Y" (quoted or bare
   words) and X occurs exactly once in the open file's editor
   context, the host builds the exact-edit proposal itself, clearly
   labelled via a Note event on the step card. A genuinely ambiguous
   or multi-occurrence request still errors - the host never guesses.

## run_command result rows, empty final answers, elapsed runs (item 230, host QA on 4406e2b)

An approved "run ls -la" worked end to end, but the step card said
"Result - read completed", an empty final model answer failed the
run as "Empty agent answer", and the Running row never showed
elapsed time. Fixes:

1. ToolResult events now carry a summary label ("ran ls -la (exit
   0)", "read src/a.php") plus the first 40 lines of a successful
   result (each clipped to 60 chars, with a "[N more lines not
   shown]" note), so the owner sees the actual command output.
   Failures keep the reason text, which includes non-zero exits
   ("exit 1: ...").

2. An empty final answer after tools actually ran is replaced with
   "Done - the command output is in the progress steps above."
   instead of erroring; a silent FIRST answer still errors.

3. Running rows show live elapsed time ("Act · 07:06 +2m 05s - run
   ls -la"): RunEntry carries started_at, and a one-second re-arming
   timer bumps a tick signal only while runs are live.
