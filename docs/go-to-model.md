# Go to Model

"Go to Model" in the command palette lists Eloquent models and their relationships and opens the one you pick.

- `model  User  (app/Models/User.php)`: opens the class line.
- `relation  User::posts  hasMany Post`: opens the relationship method.

A class is a model when it extends `Model`, `Authenticatable`, `Pivot` or `MorphPivot`. A relationship is a method whose `return $this->hasOne|hasMany|belongsTo|belongsToMany|hasOneThrough|hasManyThrough|morphOne|morphMany|morphTo|morphToMany|morphedByMany(...)` is on one line. Scans `app/Models` (or `app/` when it does not exist).

Limits: static text scan, nothing is run. Models with another base class, relations built across several lines (`return $this\n->hasMany(...)`), relations returned through helpers, and the related class when it is not written as `Name::class` (shown without it) are missed or incomplete. Depth 6, 1500 files, 1 MB per file, 5000 entries, symlinks skipped.
