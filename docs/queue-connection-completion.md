# Queue connection completion (item 406)

`Queue::connection('re` offers the queue connections the project
declares, from the `'connections' => [...]` array of
`config/queue.php` (Laravel 12.x `QueueManager`:
`connection($name = null)`).

Only a scalar name completes; an array argument stays silent. Only
the `connections` array's top-level keys are harvested: the
`'default'` key, the sibling `'failed'` region, per-connection
settings one level deeper and commented-out connections invent
nothing, comments are masked first, and a file containing
heredoc/nowdoc is skipped whole (the item 390d guard). The receiver
must be exactly `Queue::` - the `DB::` and `Redis::` arms,
`MyQueue::connection(...)` and `$queue->connection(...)` stay
separate - and a call written inside a comment or a string literal
is not a call.

Tests: `detects_queue_connection_context`,
`offers_queue_connections`.
