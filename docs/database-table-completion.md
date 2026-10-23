# Database table completion (item 381)

Table positions in schema and query code offer the project's tables,
harvested from `Schema::create('...')` calls in
`database/migrations/*.php` (sorted file order, same bounds as the
route scan, 30 s cache):

- `Schema::table('...')`, `Schema::drop('...')`,
  `Schema::dropIfExists('...')`
- `DB::table('...')`
- foreign references: `$table->foreignId('user_id')->constrained('...')`,
  `->on('...')`
- query joins: `->join('...')`, `->leftJoin('...')`,
  `->rightJoin('...')`, `->crossJoin('...')`
- validation rules: the table parameter of `exists:` and `unique:` in
  rules strings (item 388); the column positions after the comma offer
  the named table's columns (item 390, see
  database-column-completion)

`Schema::create('...')` names a NEW table, so it stays silent, as do
bare `table('...')` calls. `Schema::connection(...)->create(...)` is
not harvested; `->on`/join names beyond tables are not distinguished
by the line-local scan.

Tests: `detects_contexts` DbTable pins, `offers_database_tables`.
