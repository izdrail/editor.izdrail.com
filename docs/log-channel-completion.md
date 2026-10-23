# Log channel completion (item 399)

`Log::channel('sl` and `Log::driver('sl` offer the log channels the
project declares, and `Log::stack(['single', 'sl'])` completes each
entry of the stack list - from the `'channels' => [...]` array of
`config/logging.php` (Laravel 12.x `LogManager`:
`channel($channel = null)` as the `driver($driver = null)` alias,
`stack(array $channels, ...)`).

`stack` takes an array, so a scalar `Log::stack('...')` stays silent.
Only the `channels` array's top-level keys are harvested: the
`'default'` key, per-channel settings one level deeper (`'driver'`,
`'level'`, ...) and commented-out channels invent nothing, comments
are masked first, and a file containing heredoc/nowdoc is skipped
whole (the item 390d guard). The receiver must be exactly `Log::` -
`MyLog::channel(...)` and `$log->channel(...)` stay silent - and a
call written inside a comment or a string literal is not a call.

Tests: `detects_log_channel_context`, `offers_log_channels`.
