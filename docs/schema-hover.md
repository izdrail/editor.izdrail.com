# Table schema hover

Hovering a database table name shows the column list rebuilt from the
project's migrations, newest alters applied in order:

    DB::table('users')  ->  **`users`** - 5 columns from migrations
                            `id`, `name` string, `created_at`, `updated_at`, `bio` text
                            database/migrations/2014_10_12_000000_create_users_table.php

Recognised table references (PHP files only):

- `DB::table('users')` and the `->table(...)` builder form
- `Schema::create` / `Schema::table` / `Schema::drop` /
  `Schema::dropIfExists` / `Schema::rename` calls
- query builder `->from('users')`, `->join(...)`, `->leftJoin(...)`,
  `->rightJoin(...)`, `->crossJoin(...)`
- a model's `protected $table = 'users';` property

How the list is built: `database/migrations/*.php` are read oldest first
(file names start with a timestamp, so name order is time order).
`Schema::create` resets the list, `Schema::table` appends, and
`dropColumn` / `renameColumn` are applied. `$table->id()`,
`timestamps()`, `softDeletes()`, `rememberToken()` and the `morphs()`
family expand to their real column names; other calls show as
`` `name` method `` (e.g. `` `bio` text ``). A later `Schema::drop` /
`dropIfExists` is reported instead of the columns.

Limits: static text scan - nothing is executed and the live database is
never asked, so columns added outside migrations (or by packages) are not
shown. Bounded: depth 2, 300 files, 512 KB per file, 120 lines per
migration closure, 64 columns kept (24 shown). The first `Schema::create`
file is cited as the source.
