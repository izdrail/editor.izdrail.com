# Redis connection completion (item 405)

`Redis::connection('de` offers the Redis connections the project
declares. Laravel 12.x `RedisManager::resolve` answers the name two
ways: a top-level key of the `'redis'` region of
`config/database.php`, or a name inside its `'clusters'` sub-region
(`resolveCluster`) - so both are offered.

The region's metadata keys are not connections and are excluded:
`'client'` (a scalar), `'options'` (defaults, an array), `'clusters'`
itself, and the `'options'` metadata inside `'clusters'`. A
`'clusters'` key outside the `'redis'` region is never read. Only
top-level keys of each region are harvested: per-connection settings
one level deeper and commented-out connections invent nothing,
comments are masked first, and a file containing heredoc/nowdoc is
skipped whole (the item 390d guard). The receiver must be exactly
`Redis::` - `MyRedis::connection(...)`, `$redis->connection(...)`
and the `DB::connection` arm stay separate - and a call written
inside a comment or a string literal is not a call.

Implementation note: the region machinery (`find_region`,
`region_end`) is shared with the item 399 `config_named_keys`
harvest, which now runs on the same helpers.

Tests: `detects_redis_connection_context`,
`offers_redis_connections`.
