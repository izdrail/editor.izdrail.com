# Broadcast connection completion (item 407)

`Broadcast::connection('pu` and `Broadcast::driver('pu` offer the
broadcast connections the project declares, from the
`'connections' => [...]` array of `config/broadcasting.php`
(Laravel 12.x `BroadcastManager`: `connection($driver = null)`,
`driver($name = null)` as its alias).

Only a scalar name completes; an array argument stays silent. Only
the `connections` array's top-level keys are harvested: the
`'default'` key, per-connection settings one level deeper and
commented-out connections invent nothing, comments are masked
first, and a file containing heredoc/nowdoc is skipped whole (the
item 390d guard). The receiver must be exactly `Broadcast::` - the
other connection and driver arms, `MyBroadcast::connection(...)`
and `$broadcast->connection(...)` stay separate - and a call
written inside a comment or a string literal is not a call.

Tests: `detects_broadcast_connection_context`,
`offers_broadcast_connections`.
