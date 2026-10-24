# Database connection completion (item 404)

`DB::connection('my` offers the connections the project declares,
from the `'connections' => [...]` array of `config/database.php`
(Laravel 12.x `DatabaseManager`: `connection($name = null)`).

Only a scalar name completes; an array argument stays silent. Only
the `connections` array's top-level keys are harvested: the
`'default'` key, the sibling `'redis'` region, per-connection
settings one level deeper and commented-out connections invent
nothing, comments are masked first, and a file containing
heredoc/nowdoc is skipped whole (the item 390d guard). The receiver
must be exactly `DB::` - `Schema::connection('my`,
`MyDB::connection(...)` and `$db->connection(...)` stay silent - and
a call written inside a comment or a string literal is not a call.

Tests: `detects_db_connection_context`, `offers_db_connections`.
