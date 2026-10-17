# Middleware alias hover

Hovering a middleware alias shows the class it maps to, from the same
resolution as go-to-definition (they always agree):

    Route::middleware(['auth', 'verified'])
        -> **`auth`** - middleware alias -> `App\Http\Middleware\Authenticate`
           app/Http/Kernel.php:4

    ->middleware('admin')
        -> **`admin`** - middleware alias -> `AdminOnly`
           bootstrap/app.php:5

Works in `->middleware(...)`, `Route::middleware(...)`,
`->withoutMiddleware(...)`; a `:parameters` suffix (`throttle:60,1`) is
ignored, matching the go-to. Aliases are looked up in `bootstrap/app.php`
(Laravel 11+ `->alias([...])`) and `app/Http/Kernel.php`
(`$middlewareAliases` / `$routeMiddleware`).

Not covered (same limits as the go-to): class-name arguments, aliases
registered by the framework or packages, and anything the static scan
cannot see. Nothing is executed; files are read as text, capped at 1 MB.
