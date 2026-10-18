# Behavior tests for the agent tool surface (item 185)

Two seams extracted from the chat worker into plain functions so the
approval and symbol paths get real behavior tests:

- `agent_exec::await_approval` - the Allow once / Deny bridge. Tests
  prove: Allow returns approved; Deny returns denied; a dropped card
  counts as denial (never a hang); an unanswered card times out; a
  cancelled run errors out instead of running the command.
- `symbol_rows_json` (chat panel) - the LSP workspace-symbol result
  renderer. Tests prove: paths inside the scoped root are relativized,
  paths outside keep the raw URI (never a fake relative path), lines
  are 1-based, the result caps at 100 rows with a truncated flag, and
  empty results round-trip.

The interactive card render and the live LSP roundtrip stay
host-verified.
