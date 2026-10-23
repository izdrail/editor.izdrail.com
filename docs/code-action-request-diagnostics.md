# codeAction requests send the caret's diagnostics (item 364)

LSP codeAction requests carry a `context.diagnostics` list, and servers
(rust-analyzer included) match their quick fixes to exactly those
diagnostics. The editor collected them with
`iter_chunks(offset..offset)`: an empty chunk range yields nothing, so
the context went out empty every time and a caret on a warning got a
null/empty response even though `textDocument/publishDiagnostics` and
hover worked. Host A/B showed null codeAction results on builds with
and without unrelated popup changes, which led here.

The collection now uses `containing_spans`: the chunk range is widened
to one byte on each side of the caret (an empty range yields nothing),
then filtered to spans containing the caret, boundary-inclusive. The
cache needs no change: `clear_code_actions` already runs when
diagnostics are published and on every edit, so a request made before
publishDiagnostics is re-issued with the diagnostics attached.

Whether a particular warning then produces an action is the server's
choice (an unused import offers "remove import"; not every diagnostic
has a fix). The client side is now spec-correct.

Tests: `code_action_context_tests` (`collects_spans_touching_the_caret`,
`handles_empty_spans_and_edge_offsets`).
