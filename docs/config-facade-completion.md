# Config facade completion (item 395)

The `Config` facade completes the same keys as the `config('...')`
helper: `Config::get('app.na`, `Config::has('app.de` and
`Config::set('app.de` all offer the project's config keys. The key is
the first argument in each (Laravel 12.x `Repository`: `get($key,
$default)`, `has($key)`, `set($key, $value)`), so the default/value
position stays silent.

The array forms differ per method (Laravel 12.x `Repository`):
`has` passes the array to `Arr::has` as a list of keys; `get` goes
through `getMany`, where numeric entries are keys and assoc keys are
keys whose assoc values are per-key defaults; `set` takes assoc
`key => value` pairs only - a numeric-list string has no meaning
there and is not claimed as a key. A string completes only when it
opens after `[` or `,` at depth exactly 1, the outer key array -
which for `set` is the start of an assoc key the user is typing.
Assoc values never complete: `Config::set(['app.name' =>
'arbitrary'])` and `Config::get(['app.name' => 'fallback'])` offer
nothing at the value. Nested arrays are value/default data, not key
lists: `Config::set(['app.options' => ['nested']])` and
`Config::get(['app.key' => ['default']])` stay silent inside the
inner brackets.

The receiver must be exactly `Config::` - `MyConfig::get(...)` and
`$config->get(...)` stay silent - and a call written inside a comment
or a string literal is not a call. Keys come from the same
`config/*.php` harvest as the helper.

Tests: `detects_config_facade_context`, `offers_config_facade_keys`.
