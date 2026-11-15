# WordPress cron recurrence name completion

Typing a cron recurrence offers the recurrence names the project
itself schedules with (item 436).

## Where it works

- `wp_schedule_event($time, '...', $hook)` - the recurrence is the
  second argument. The hook position completes hook names instead
  (item 427).
- `wp_reschedule_event($time, '...', $hook)` - same.

## What is offered

Recurrences harvested from those calls across the project's `.php`
files (skipping `.git`, `node_modules` and `vendor`; at most 500
files, 1 MB each). Textual scan; nothing is executed. A recurrence
the project never schedules with is dead weight, so scheduling
calls alone declare the list - this covers custom recurrences
registered through the `cron_schedules` filter (`shop_hourly`) and
the core recurrences the project actually uses (`hourly`,
`twicedaily`, `daily`). Names from commented-out code, from
lookalike functions and from inside strings are not collected.
Unsaved edits are not seen until the lists cache expires.
