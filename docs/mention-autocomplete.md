# Composer @-mention autocomplete (item 197, Antigravity parity: @ context picker)

Typing `@` in the composer opens a fuzzy file picker above the input:
the workspace's files ranked by basename-prefix, then substring, then
subsequence match (top 8). Arrow keys move the highlight, Enter (or a
click) inserts `@relative/path ` in place of the partial token, Escape
dismisses. The inserted token is exactly what send-time
`resolve_mentions` (item 190) expands into inlined file context, so the
popup and the resolver can never disagree about path form.

The workspace file index loads lazily on the first `@` via the
palette's GetFiles proxy RPC and is cached as relative paths (`.git`
internals filtered). The popup only tracks a live tail token - an
`@`-token at the very end of the composer text with no trailing
whitespace - so finished mentions and email-shaped text never trigger
it, and it stays closed right after an insert. Local workspaces only
(agent reads are local).

Also fixed in passing: the mode/agent dropdown rows no longer render
unselected entries in dim grey (it read as disabled) - rows use normal
text color; only the selected row keeps a persistent highlight and the
hovered row a transient one.

Coverage: trailing_mention (tail-token rules, trailing-whitespace
close, punctuation rejection, multi-line offsets) and
mention_candidates (basename-prefix ranking, substring and subsequence
fallbacks, no-match empty, cap) are unit-tested.

## Keyboard handling (0241)

The composer's text editor consumes Enter/Tab/arrows in its own keymap
before view-level `on_event` listeners ever see them, so the first
popup build could not be driven from the keyboard: Enter inserted a
newline (which also broke the tail token and closed the popup), arrows
moved the caret, Tab did nothing.

The composer is now built with `text_editor_keys`, whose key hook runs
at the same point as the editor's own Enter-newline binding. While the
mention popup is open: Up/Down move the selection (clamped, with a
visible highlight on the selected row), Enter and Tab insert the
selected candidate, Escape closes the popup; all of these return
`CommandExecuted::Yes` so the editor never sees them. With the popup
closed every key falls through to `default_key_handler`, so normal
composer behavior (including Enter-to-newline) is unchanged.
