# symbol_search tool (LSP workspace symbols) for Ask/Plan/Act (item 184)

The agent loop gains a fourth read-only tool, `symbol_search(query)`,
registered in every mode. It bridges to the existing proxy
`get_workspace_symbols` lane (the one the palette's `#` mode uses)
from the agent worker thread with a bounded blocking wait: 10s
timeout, cancellation-aware, empty result when no language server
serves the project. Each hit returns name, kind, container, a
workspace-relative path when possible, and a 1-based line; capped at
100 symbols with a truncated flag.

Coverage: the loop's registry validation picks up the new tool; the
proxy roundtrip itself needs a live language server, so it stays
host-verified (same class as the palette path it mirrors).
