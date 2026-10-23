# PHP code actions made deterministic (item 383)

Host E2E saw `textDocument/codeAction` return nothing for PHP while hover
and diagnostics worked. Local reproduction against the pinned phpactor
(2026.10.07.0, the exact phar the registry downloads) with the proxy's exact
request shape - point range, `context.diagnostics`, injected `textDocument` -
returns correct quick fixes in under a second, so the server and the wire
format are fine. The failure comes from two silent-empty mechanisms:

1. Phpactor outsources every code action to a spawned
   `language-server:code-action` process
   (`language_server.code_action_outsource` defaults to true). The process is
   killed after 5s (`language_server.diagnostic_outsource_timeout`): a
   timeout logs a warning and returns `[]`, a non-zero exit or undecodable
   output throws, surfacing as a JSON-RPC error. Both look exactly like "no
   actions" to the client, and both leave no trace in the UI.
2. The app inserted an empty placeholder into `doc.code_actions()` before
   awaiting the response and ignored error responses entirely. One failed
   request therefore pinned the offset to "no actions" until the next edit
   or diagnostics publish - a persistent null with no retry.

Fixes:

- The phpactor registry entry now sets
  `"language_server.code_action_outsource": false`, so code actions run in
  the server process itself (verified: same quick fix returned in ~0.07s
  with this option against the pinned phar). This removes the whole
  outsourced failure class: timeouts, spawn crashes, orphan processes.
- On a failed code-action request the app now removes the placeholder and
  logs a warning, so the next caret move retries instead of reporting "no
  actions" forever. The removal is dispatched through `create_ext_action`
  because the proxy callback must stay `Send` and cannot hold `Rc<Doc>`.

GUI E2E fixture (verified against the pinned server on the wire):

```php
<?php

$greeting = 'hello world';

echo $greetign;
```

Caret on `$greetign` (last line): the lightbulb offers
"Correct undefined variable \"$greetign\" to \"$greeting\"". Choosing it
rewrites `$greetign` to `$greeting`; undo restores the original text.
Negatives: caret on `$greeting` (defined variable) offers no
correct-variable-name quick fix; a file with no diagnostics offers no
quickfix actions.
