# Middleware alias completion (item 393)

Typing a quoted string in the first argument of `->middleware(...)`,
`Route::middleware(...)` or `->withoutMiddleware(...)` offers the
middleware aliases and groups the project declares:

```php
Route::get('/admin', $c)->middleware('ad  // narrows to `admin`
Route::middleware(['auth', 've            // narrows to `verified`
```

Array entries in the first argument complete too (the bracket comma
is depth, not the next argument); a second variadic argument and
class constants (`->middleware(Authenticate::class)`) stay silent, as
does the `alias:parameter` suffix - in `->middleware('throttle:60,1')`
the part after the colon is a value, not an alias. `MyRoute::` and
other look-alike static receivers are not `Route::`.

## Where the names come from

Only the declaration arrays are scanned: the top-level
`'name' => ...` keys of each `->alias([...])` array in
`bootstrap/app.php` (Laravel 11+) and of the `$middlewareAliases`,
`$routeMiddleware` and `$middlewareGroups` property arrays in
`app/Http/Kernel.php` (so group names such as `web` complete too).
Comments are masked and string literals mapped first, so a
commented-out alias is not offered, an `->alias(` inside a string
value is not a declaration, a `->alias($variable)` call without an
array literal declares nothing, keys of nested arrays are not
aliases, and an unrelated `'name' => ...` config entry elsewhere in
either file invents nothing. Files containing heredoc/nowdoc (`<<<`)
are skipped whole, the same fail-closed guard as item 390d. Aliases
the framework or a package registers itself (`auth`, `guest`,
`throttle`, ... without a project declaration) are not offered.

Alias names can contain dots and dashes, so the context joins the
dotted-name word extension: accepting a completion replaces the whole
name.

Tests: `detects_middleware_alias_context`, `offers_middleware_aliases`.
