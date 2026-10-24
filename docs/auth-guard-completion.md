# Auth guard completion (item 402)

`Auth::guard('we` and `Auth::shouldUse('we` offer the guards the
project declares, from the `'guards' => [...]` array of
`config/auth.php` (Laravel 12.x `AuthManager`: `guard($name =
null)`, `shouldUse($name)`).

Only a scalar name completes; an array argument stays silent. Only
the `guards` array's top-level keys are harvested: the nested
`'defaults' => ['guard' => ...]` key, per-guard settings one level
deeper and commented-out guards invent nothing, comments are masked
first, and a file containing heredoc/nowdoc is skipped whole (the
item 390d guard). The receiver must be exactly `Auth::` -
`MyAuth::guard(...)` and `$auth->guard(...)` stay silent - and a
call written inside a comment or a string literal is not a call.

Tests: `detects_auth_guard_context`, `offers_auth_guards`.
