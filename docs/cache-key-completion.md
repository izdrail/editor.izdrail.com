# Cache key completion (item 415)

`cache('re` and the key-first `Cache::` methods (`get`, `pull`,
`has`, `missing`, `add`, `forever`, `remember`, `rememberForever`,
`sear`, `flexible`, `increment`, `decrement`, `set`, `delete`,
`forget`) offer the cache keys the project writes: the scalar names
of `Cache::put/forever/add/set/remember/rememberForever/sear/
flexible` and their `cache()->...` / `->cache()->...` chain forms,
plus the assoc keys of `Cache::put([...])` / `Cache::putMany([...])
` / `Cache::setMultiple([...])` and the first assoc key of
`cache(['key' => ...], $ttl)` - the helper stores only the first
pair of an array argument (Laravel 12.x `helpers.php:281`,
`key($key)`), and `Repository::put` delegates an array key to
`putMany` (`Repository.php:347` -> `:398`).

The array-accepting methods (`put`, `many`, `getMultiple`,
`deleteMultiple`, `putMany`, `setMultiple`) also complete strings
inside their first-argument array at depth exactly 1; scalar-only
methods stay silent there. A second argument is a value, ttl or
default and stays silent, as do look-alike receivers
(`MyCache::get(...)`), object calls (`$cache->get(...)`), the
`cache()->get(...)` and `Cache::store('redis')->get(...)` chains,
and calls written inside a comment or a string literal. The scan
covers `app/` and `routes/`, masks comments and string literals
and skips files using heredoc/nowdoc, so a commented-out or
in-string put declares nothing.

Tests: `detects_cache_key_context`, `offers_cache_keys`, plus
`name_replacement_kinds_are_php_file_only` pins.
