# Eloquent relation completion (item 418)

`Post::with('com` and the eager-load family - `->with('...')`,
`->load('...')`, `->without('...')`, `->withCount('...')`,
`->loadCount('...')`, `->loadMissing('...')`, and the
`withSum` / `withAvg` / `withMin` / `withMax` / `loadSum` / `loadAvg`
twins - offer the relation methods of the model class named earlier on
the line with `Class::`. A relation is a method whose body starts with
`return $this->hasOne|hasMany|belongsTo|belongsToMany|hasOneThrough|
hasManyThrough|morphOne|morphMany|morphTo|morphToMany|morphedByMany(...)`,
parsed by the model index; scopes and private helpers are not relations.

The bound class is the LAST `Class::` token before the open paren
(byte order), so `Post::where('x', 1)->with('` still binds Post and
`Post::with('a')->get(); Comment::with('` binds Comment. The model
file is found by file stem under `app/Models` (or `app/` when that
folder is missing); a class with no model file offers nothing.
`Redirect::route(...)->with('flash'` and `View::make(...)->with('key'`
take flash / view data, not relation names, and stay silent, as do a
bare `with(` call, a second argument, and `withTrashed` /
`withAttributes` look-alikes. The array form `with(['com` completes the
same names; a string inside a closure value (`with(['c' => fn ($q) =>
$q->where('...')])`) can still raise the list, the same ambiguity the
route-parameter arm accepts. `$post->load('` binds nothing (the
variable's class is unknown statically) - name the class on the line.
Static text scan, nothing executed, cached 30 s per workspace + class.
