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
