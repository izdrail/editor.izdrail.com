# Cache store completion (item 400)

`Cache::store('re` and `Cache::driver('re` offer the cache stores
the project declares, from the `'stores' => [...]` array of
`config/cache.php` (Laravel 12.x `CacheManager`: `store($name =
null)`, `driver($driver = null)` as its alias).

An array argument to `store` is an on-demand config, not a name
list, so `Cache::store(['dri` stays silent. Only the `stores`
array's top-level keys are harvested: the `'default'` key, per-store
settings one level deeper and commented-out stores invent nothing,
comments are masked first, and a file containing heredoc/nowdoc is
skipped whole (the item 390d guard). The receiver must be exactly
`Cache::` - `MyCache::store(...)` and `$cache->store(...)` stay
silent - and a call written inside a comment or a string literal is
not a call.

Tests: `detects_cache_store_context`, `offers_cache_stores`.
