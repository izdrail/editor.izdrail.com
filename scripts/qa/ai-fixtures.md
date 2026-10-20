# Isolated AI UI fixtures

Use a disposable editor profile/project, not the owner's saved profile. Do not
put real keys in commands, screenshots, or fixtures. No model downloads needed.

## Masked field rendering

Debug build only: launch with `LAPCE_UI_FIXTURE=masked-key` and open Settings >
AI Chat. The new key field contains fixed dummy text and must paint only dots
plus the unsaved-entry label. This does not prove typing, paste, focus, or
backspace works. Release builds ignore the variable. Do not save the dummy key
for a real provider. Close the editor after capturing.

## Connection feedback and stale result

Run `python3 scripts/qa/ai_fixture_server.py --delay 8 --status 200`.
In the disposable profile, set endpoint to `http://127.0.0.1:18761` and model to
`fixture-model`. Leave the new-key field empty. Test connection, change model
before eight seconds, and capture stale feedback. With unchanged fields it
must show OK. Restart server with `--status 401` to test unauthorized feedback;
`--delay 25` exceeds the editor's 20-second timeout. No quota or real service
is involved. Server logs no headers, bodies or keys. Do not run masked-key mode
for this test because an unsaved key deliberately blocks testing.

This server supports the non-streaming connection probe and model discovery,
not chat SSE/tool calls. It is not yet a fixture for an actual Agent conversation.

## PHP project entry

Create a disposable folder with `app/Example.php` containing:

```php
<?php
namespace App;
class Example { public function greet(): string { return 'fixture'; } }
```

Launch `gravity-editor --new /absolute/path/to/project` (substitute the built
binary's name). CLI also accepts `/absolute/path/to/project/app/Example.php:3:1`.
Use command palette `Toggle AI Chat` to open the chat panel. These are existing
CLI/command routes, not automatic UI interactions. A PHP file fixture is not
proof a PHP language server is installed or that its symbols are rendered.

Still required: harness keyboard/focus repair for actual key entry, in-flight
field edits, completion popup, and symbol UI interaction. Synthetic screenshots
must never be reported as those interaction checks passing.

## Ghost-text and keymap-aware status hint (item 279)

Start `python3 scripts/qa/ai_fixture_server.py --inline`. In a disposable profile:

```toml
[ai-autocomplete]
enabled = true
backend = "ollama"
endpoint = "http://127.0.0.1:18761"
remote-model = "fixture-model"
auto-trigger = true
auto-trigger-debounce-ms = 400
next-edit = false
```

Open a real saved source file, enter insert mode, type a character and pause,
or invoke `inline_completion.invoke` via its configured binding (default Alt+\).
The native worker sends /api/generate; --inline returns the fixed NDJSON text
`fixtureCompletion()`. No download, LLM, token or real service is used. Disable
other completion popups if they suppress automatic ghost text. Focus must be
Workbench, active document must match, and suggestion must be active for hint.

Capture at 1600px and 900px, then use a disposable keymaps file to unbind default
Tab/Esc and bind inline_completion.select/cancel to Ctrl+J/Ctrl+K. Verify the
actual loaded mappings in keymap UI before capture. Accept/dismiss must follow
the displayed sequence. Undo accepted fixture text; never use this profile for
real code. --inline is off by default and does not alter editor product behavior.
The server still never logs request bodies or keys. With --status 401 it returns
that error even for /api/generate. This is transport response fixture coverage,
not proof that auto-trigger, hint matching or paint passes on the host.
