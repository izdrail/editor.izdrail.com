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
Laravel convention, same as item 382), and heredoc/nowdoc strings are
not masked.

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

Tests: `offers_validation_rule_columns`,
`harvest_ignores_comments_strings_and_stray_calls`, plus the
`detects_validation_rule_context` DbColumn pins.
