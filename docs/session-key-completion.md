# Session key completion (item 414)

`session('th` and the key-first `Session::` methods
(`Session::get/put/flash/forget/exists/missing/has/hasAny/pull/
remember/push/now/increment/decrement`) offer the session keys the
project writes: the scalar names of `Session::put/flash/now/push`,
`->session()->put/flash` and `session()->put/flash`, and the assoc
keys of `session(['key' => ...])` / `Session::put(['key' => ...])`,
scanned in `app/` and `routes/` (Laravel 12.x `helpers.php:910`
`session($key, $default)` - an array first argument puts, anything
else gets - and the `Session\Store` key-first signatures). Reads
declare nothing; a key exists in the list only after some code puts
it.

The array-accepting methods (`put`, `forget`, `exists`, `missing`,
`has`, `hasAny`) also complete strings inside their first-argument
array at depth exactly 1; scalar-only methods stay silent there. A
second argument is a value or default and stays silent, as do
look-alike receivers (`MySession::get(...)`), object calls
(`$session->get(...)`), the `session()->get(...)` chain, and calls
written inside a comment or a string literal. The scan masks
comments and string literals and skips files using heredoc/nowdoc,
so a commented-out or in-string put declares nothing.

Tests: `detects_session_key_context`, `offers_session_keys`, plus
`name_replacement_kinds_are_php_file_only` pins.
