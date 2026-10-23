# Blueprint method completion (item 382)

Inside a migration closure, `$table->` offers the Laravel Blueprint
methods: the 12.x column types (`string`, `foreignId`, `timestamps`,
`ulidMorphs`, `vector`, ...), the index methods (`primary`, `unique`,
`fullText`, `spatialIndex`, `foreign`) and the table commands
(`dropColumn`, `renameColumn`, `dropSoftDeletes`, ...). 83 entries,
cross-checked against the Laravel 12.x migration docs and
`Illuminate\Database\Schema\Blueprint` (`nullableTimestamps` and the
nullable morph helpers exist in the framework but not the docs list).

The context is line-local: the caret must sit right after `$table->`
with only name characters since, in a file under
`database/migrations/`. The file restriction is what keeps the
variable-name convention honest: an unrelated `$table` object in a
controller or model sees no Blueprint methods. Arguments
(`$table->string('na`) and chains (`$table->string('x')->nul`) are not
name positions - chain modifiers (`->nullable()`, `->default()`,
`->change()`) are out of scope. `$this->table->...` does not contain
the `$table->` marker and stays silent. A differently named closure
parameter (`$t`) is not detected.

Tests: `detects_contexts` Blueprint pins, `offers_blueprint_methods`.
