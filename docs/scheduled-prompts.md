# Scheduled AI prompts

The AI Chat panel can fire prompts on a schedule: an hourly review, a
morning standup question, a periodic diff summary. Click **Schedules** in
the chat panel header to open (or create) `.lapce/scheduled-prompts.json`
in the open folder and edit the list:

```json
{
  "prompts": [
    { "prompt": "Summarize what changed in the open file", "every_minutes": 120 },
    { "prompt": "List the TODO comments in this project", "at": "09:00" }
  ]
}
```

Two shapes per entry: `every_minutes` (interval, clamped to 5 minutes
through one week) or `at` (once a day at local HH:MM). The editor checks
once a minute and fires one due prompt per tick into the chat lane,
exactly as if you had typed it.

Honest limits: firing happens only while the window is open and the chat
backend is enabled; a busy chat or a closed editor postpones, never
bursts. The last fire time is written back to the file before sending, so
a crash cannot double-fire. Malformed entries are dropped on read; times
that are not valid HH:MM never fire. The file lives in the workspace, so
schedules are per project and safe to commit or ignore.

Bounds: at most 32 entries, prompts clipped at 4 KB on a char boundary.
