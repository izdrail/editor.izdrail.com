# `@path` composer mentions (item 190, Antigravity parity: context mentions)

Typing `@relative/path` in the chat composer attaches that file to the
turn: the panel resolves each mention through the scoped, size-capped
ReadRoot lane and appends the contents to the WIRE prompt as
`<mentioned_file path="...">` blocks (marked untrusted data), while
the message shown in the chat keeps the user's original text. Works in
all three lanes: plain Chat, the Ask/Plan/Act agent loop, and ACP
agents. A mention is `@` at a word boundary followed by a path-like
token (letters, digits, `_./-`, containing `/` or `.`); emails and
Rust decorators are left alone, at most 8 unique files per message,
256 KiB total budget. The You-turn context line lists what was
attached; unresolvable mentions stay literal and are named in the
panel error line.

Coverage: extraction rules (path-like only, punctuation, dedupe, cap)
and real-FS resolution (attach order, fences, failure reporting,
`..` escape refusal, passthrough) are behavior-tested.

`@https://...` / `@http://...` tokens are web mentions (item 203):
Ask/Plan/Act runs fetch and inline the page on the worker thread; see
docs/agent-web-fetch.md. Plain Chat passes them through as text.
