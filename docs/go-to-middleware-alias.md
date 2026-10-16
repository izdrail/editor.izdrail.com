# Go to Laravel middleware alias

In a PHP file, put the cursor on an alias inside `->middleware(...)`, `Route::middleware(...)` or
`->withoutMiddleware(...)` and run Go to Definition:

    Route::middleware(['auth', 'verified'])->group(...);   // cursor on 'auth'
    ->middleware('throttle:60,1')                          // parameters after ':' are ignored

It opens the `'auth' => ...` line in `bootstrap/app.php` (Laravel 11+ `->alias([...])`) or
`app/Http/Kernel.php` (`$middlewareAliases` / `$routeMiddleware`), whichever declares it.

Not resolved: class names (`Auth::class`) and names containing a backslash, which normal Go to
Definition handles, and aliases registered by the framework or a package (for example `auth`
on a fresh Laravel 11 install lives in the framework). Limit: 1 MB per file. Nothing is executed.
If nothing is found, normal Go to Definition runs.
