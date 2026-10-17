# Project-aware completion

Typing inside one of these calls offers the project's own names in the
completion popup (a "route name" / "config key" / "env key" / "Blade view"
detail says which), next to the LSP, snippet, keyword and buffer-word
results:

    route('|        -> users.index, home, ...   (from ->name(...) in routes/*.php)
    config('|       -> app.name, mail.from.address, ...   (config/*.php)
    env('|          -> APP_NAME, DB_HOST, ...   (.env files, names only)
    view('|         -> pages.home, emails.welcome, ...   (resources/views)

Also recognised: `to_route(`, `->routeIs(`, `Route::has(` (but not
`$collection->has(`), and the Symfony route generators `path(`,
`generateUrl(`, `redirectToRoute(`, `generate(`.

How it works: the same static scans as the Go to Route / Env / Config Key
palettes - nothing is executed. Results are cached for 30 seconds per
workspace so completion stays cheap on every keystroke; new routes, keys or
views appear within that window.

Limits: Laravel route names come from explicit `->name('...')` calls only
(resource implicit names and group `->name('prefix.')` assembly are not
offered, matching the go-to limits). Symfony route names are not offered
yet. Bounded: 60 route files, 1000 view files, 1 MB per file, 2000 names,
500 items offered. Quote escapes (`route('it\'s')`) fall out of context.
