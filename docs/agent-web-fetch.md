# Agent web_fetch tool (item 201)

Parity item 5: the agent loop had no web access at all, so models could
not read documentation, release notes or issue pages while answering.
Antigravity exposes the same idea through its browser/@web context.

`web_fetch` is registered for every agent mode (Ask, Plan and Act) with
one required string argument, `url`. The agent loop's existing trust
gate covers it: the tool only runs after the owner approved project
reads for the window, and fetched content is fed back to the model
labelled as untrusted data, same as file reads.

## Bounds and refusals

- http(s) only; other schemes are refused.
- Localhost names (`.localhost`, `.local`, `.internal`) and literal IPs
  in loopback, private, link-local, CGNAT, multicast, documentation and
  unspecified ranges are refused, so the tool cannot be pointed at the
  machine's own services. A public hostname that resolves to a private
  address is NOT detected in v1 (no DNS re-check) - that is a documented
  limitation, not a guarantee.
- Redirects are refused outright; the error names the redirect target so
  the model can ask for the final URL itself. The tool never follows a
  redirect target the model did not request.
- Only text content types are returned (`text/*`, JSON, XML,
  JavaScript). Binary or unknown types are refused.
- Wire read is capped at 1 MiB; returned text is capped at 64 KiB with
  an explicit truncation marker. Connect timeout 10 s, total 30 s.
- HTML is stripped to plain text (script/style/template/noscript blocks
  dropped, block tags become line breaks, common entities decoded). It
  is not a browser rendering, but it is enough for documentation pages.

`agent_web::host_is_local`, the URL checks and the HTML-to-text pass are
unit-tested; live fetches are covered by the host gate's manual runs.

## Follow-ups (not in this slice)

- DNS-resolution checks against private ranges.

## @web composer mentions (item 203)

Typing `@https://...` (or `@http://...`) in the composer inlines the
fetched page as a `<mentioned_page>` block for Ask/Plan/Act runs,
sharing the 256 KiB mention budget with `@file`. Fetches run on the
agent worker thread (they block for seconds); the UI thread never
fetches. Attached and failed pages show as step rows in the run's
progress card. Plain Chat sends the token as literal text - the
answer-only lanes have no fetch lane. Up to four pages per message;
sentence-trailing punctuation is trimmed like `@file` mentions.
