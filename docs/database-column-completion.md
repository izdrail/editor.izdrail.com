# Database column completion (item 390)

Inside the `exists:` / `unique:` validation rules, the column
positions offer the columns of the table named before the first
comma: `'required|exists:users,em` narrows to `email`.

The parameter grammar is grounded in Laravel 12.x
`ValidatesAttributes::validateExists` / `validateUnique` /
`getExtraConditions`:

- `exists:table,column,column,value,...` - index 1 is the verified
  column; from index 2 the parameters alternate condition column,
  condition value. Columns complete at indices 1, 2, 4, 6, ...
- `unique:table,column,except,idColumn,column,value,...` - index 2 is
  the except VALUE (not a column), index 3 is the id column; from
  index 4 the parameters alternate condition column, condition value.
  Columns complete at indices 1, 3, 4, 6, 8, ...

Value positions (the except value, the condition values) and an empty
table stay silent.

## Query-builder chains (item 392)

When a call's own chain is based on a literal `DB::table('...')`
earlier on the same line, a column-name first argument of a
query-builder call offers that table's columns:
`DB::table('users')->where('em` narrows to `email`, and the same for
`orWhere`, `whereIn`, `whereNotIn`, `whereNull`, `whereNotNull`,
`whereBetween`, `orderBy`, `groupBy`, `select`, `pluck`, `value`,
`latest` and `oldest`.

The receiver is resolved by walking backward over whole
`->method(...)` segments, so scope is exact: inside a nested chain
the inner base decides
(`DB::table('users')->whereIn('id', DB::table('posts')->select('`
offers `posts` columns), and once the subquery closes the outer base
decides again (`...->select('u'))->where('` offers `users` columns).

Line-local, like every context here: `Model::where('...')`,
table-less chains, and receivers that reach a statement boundary or
anything but a literal `DB::table` base
(`$x = DB::table('users')->count(); $q->where('` stays silent) never
inherit a table - no model-to-table or cross-statement guessing. The
operator/value arguments after the first are not column positions.

## Fluent `Rule::exists` / `Rule::unique` (item 394)

The fluent rule builders take the same pair as plain arguments
(Laravel 12.x: `Rule::exists($table, $column)` /
`Rule::unique($table, $column)`), and both positions complete:
`Rule::exists('us` / `Rule::unique('us` offer table names, and
`Rule::exists('users', 'em` offers the columns of the table quoted in
the first argument. The table is read from the current call's own
opening parenthesis, so an earlier `Rule::` mention on the line never
donates its table to the current call. `->exists(...)` on a query
builder, a look-alike `MyRule::exists(...)`, a call written inside a
comment or a string, a table given as a variable, and the
non-existent third argument stay silent.

## Where the columns come from

A static text scan of `database/migrations/*.php` (same bounds as the
table scan: 60 files, 1 MB each, 2000 pairs, 30 s cache). Comments
(`//`, `/* ... */`, `#`) are blanked and string literals mapped before
scanning, so commented-out or in-string `Schema::` / `$table->` text
produces nothing. Each `$table->` call is attributed only inside the
enclosing `Schema::create/table` closure body - a block
(`function (...) { ... }`) or arrow (`fn (...) => ...`) closure; a
call outside any schema closure is ignored, and another `Schema::`
operation (`dropIfExists` / `drop` / `rename`) ends the attribution.
A closure parameter named other than `$table` is not seen (the
Laravel convention, same as item 382). A file containing heredoc/nowdoc
(`<<<`) is skipped whole: the scan does not mask those strings, so it
fails closed - missing suggestions, never invented columns.

Harvested: column-type Blueprint methods (the item 382 list) with an
explicit first name argument (`$table->string('name')` -> `name`),
plus the implicit columns of `id()`, the `timestamps()` family
(`created_at`, `updated_at`), `softDeletes()` (`deleted_at`),
`rememberToken()` (`remember_token`) and the `morphs` family
(`$table->morphs('imageable')` -> `imageable_id`, `imageable_type`).

## What stays silent

Value positions and unknown/empty tables (above). Index, foreign,
drop and rename commands are not harvested, so a column dropped by a
later migration can linger. `Rule::exists(...)` / `Rule::unique(...)`
are fluent PHP calls, not rule strings, and stay out.

Tests: `offers_fluent_rule_tables_and_columns`,
`detects_fluent_rule_context`, `offers_query_columns`,
`detects_query_column_context`,
`offers_validation_rule_columns`,
`harvest_ignores_comments_strings_and_stray_calls`,
`harvest_fails_closed_on_heredoc`, plus the
`detects_validation_rule_context` DbColumn pins.
