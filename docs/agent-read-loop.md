# Read-only Ask/Plan loop contract (milestone 5a)

agent_loop supplies a bounded single-agent coordinator for explicitly configured
provider/executor callbacks. It is not connected to the editor UI yet. It cannot
choose a provider, obtain keys, select roots, grant trust or apply changes.

Ask answers from bounded project reads; Plan returns an unapproved plan. Act fails
closed until reviewed-edit UI and application exist. Default limits are eight
model steps, sixteen tool calls and 180 seconds, checked between operations.
Provider transport must enforce its own cancellation/timeouts; a blocking callback
cannot be interrupted by the pure loop alone. All execution must stay off UI.

Only read_file and search schemas exist. Every proposed batch is validated before
execution, including registered names, correlated IDs, argument schema and limits.
Tool errors return explicit error data, not fabricated success. File content and
results are untrusted data. The policy/executor independently bounds access; model
instructions alone are not a security boundary. Time/step/call exhaustion is an
error, never a completed task. Paid adapters remain disabled.

Next integration: explicit workspace-trust UI, Ask/Plan mode selector, live run
steps/results, stop token passed into real provider and execution worker, artifact
plan display; then reviewed-edit UI for Act. No end-to-end agent capability is
claimed before those pieces and real model/pixel tests pass.

Verification: four pure coordinator tests cover tool-result routing, Plan not Act,
unknown-tool rejection, cancellation/step limits, error data and invalid arguments.
Full local app check passes. Host code/build tests and GUI verification still apply.

## Linux UI integration (5b)

The chat panel now has Chat, Ask and Plan choices. Ask/Plan require a local Linux
project, an explicitly configured tool-capable Ollama backend, and the owner
clicking Approve Project Reads for this window after the endpoint disclosure.
Approval is memory-only and bound to the exact configured endpoint. Changing that
endpoint requires another approval. Revoke invalidates the run and aborts its
request; the server may still complete inference after client disconnect.

Chat remains the default and preserves existing local behavior. Local Candle
agent tool messages are not supported and no hosted fallback is used. Ask/Plan
run in a worker thread with the bounded read/search executor and tool-aware Ollama
transport. Live step/tool status appears in the answer slot; the final answer/plan
replaces it. No artifact panel, persistent agent tool transcript or multi-turn agent
history is included yet. Each Ask/Plan invocation starts a bounded fresh run.

Act remains unavailable, and no files are written. The UI does not claim model
quality parity. A real tool-capable model on the configured endpoint and interactive
pixel/Stop/trust tests are still required. Compilation and loopback mocks are not
that evidence. Existing prompts/replies history remains opt-in.

## Plan artifacts (item 138)

When a Plan-mode turn finishes, the reply is also written to
`.lapce/plans/plan-<unix-timestamp>.md` in the project and opened as a
normal editor buffer: title, the original request, the plan, and a Next
section. Feedback means editing that file; proceeding means asking the
agent (Act mode still waits on its review UI QA). The chat turn is
unaffected and history persistence is unchanged. Ask mode writes no
artifact. Filenames use unix seconds so no clock formatting or chrono
dependency is involved; a same-second second plan overwrites the first
(acceptable for a manual flow).
