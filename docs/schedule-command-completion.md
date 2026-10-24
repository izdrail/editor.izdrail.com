# Scheduled command completion (item 413)

`Schedule::command('re` offers the same artisan command names as
`Artisan::call` / `Artisan::queue` (Laravel 12.x `Facades\Schedule`
`@method command($command, ...)` -> `Scheduling\Schedule::command`
:157) - the project's custom commands and the built-in list, with
the item 412 provenance detail and the item 410 whole-name
replacement, since it resolves to the same completion kind.

Only a scalar name completes; the array-parameters second argument
and the array form stay silent. The receiver must be exactly
`Schedule::` - `MySchedule::command(...)` and the
`$schedule->command(...)` object call stay silent - and a call
written inside a comment or a string literal is not a call.

Tests: `detects_schedule_command_context`,
`offers_artisan_commands` (schedule pins).
