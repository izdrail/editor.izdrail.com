# regex_search tool for Ask/Plan/Act (item 183)

The agent loop gains a third read-only tool, `regex_search(pattern)`,
registered in every mode: bounded per-line regex search over the same
scoped, gitignore-aware, secret/deny-listed project walk as literal
search. Patterns are single-line, up to 256 bytes, compiled with a
1 MB size limit; results share the literal tool's caps (100 matches,
8 MiB total, 2s wall clock, 1024-char lines, char-boundary safe) and
the same {"matches","truncated","skipped_files"} response envelope.

Coverage: agent_read test (per-line match, gitignore respected,
invalid/newline/empty patterns rejected); the loop's arg registry and
per-step cards pick up `pattern` like the other tools.
