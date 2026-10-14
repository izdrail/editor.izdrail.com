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
