# Eloquent relation completion (item 418)

`Post::with('com` and the eager-load family - `->with('...')`,
`->load('...')`, `->without('...')`, `->withCount('...')`,
`->loadCount('...')`, `->loadMissing('...')`, and the
`withSum` / `withAvg` / `withMin` / `withMax` / `loadSum` / `loadAvg`
twins - offer the relation methods of the model class named earlier on
the line with `Class::`. A relation is a method whose body starts with
`return $this->hasOne|hasMany|belongsTo|belongsToMany|hasOneThrough|
hasManyThrough|morphOne|morphMany|morphTo|morphToMany|morphedByMany(...)`,
Parsed by the existing line-based model index. It does not resolve inherited methods, imports, aliases or nested relationship paths.

The bound class is the outer static receiver of the current statement. Class names inside arguments, strings, comments or earlier statements cannot donate a receiver. `Post::where('x', User::class)->with('` binds Post. The model
file is found by file stem under `app/Models` (or `app/` when that
folder is missing); a class with no model file offers nothing.
`Redirect::route(...)->with('flash'` and `View::make(...)->with('key'`
take flash / view data, not relation names, and stay silent, as do a
bare `with(` call, a second argument, and `withTrashed` /
`withAttributes` look-alikes. The array form `with(['com` completes the
same names. Array values and comment/string-contained calls stay silent. `$post->load('` binds nothing because variable types are not inferred. Items are offered only in PHP files.
Static text scan, nothing executed, cached 30 s per workspace + class.

Sources checked: https://laravel.com/framework/docs/12.x/eloquent-relationships and https://github.com/laravel/framework/blob/12.x/src/Illuminate/Database/Eloquent/Model.php
