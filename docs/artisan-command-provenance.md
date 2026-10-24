# Artisan command provenance (item 412)

The completion detail for `Artisan::call` / `Artisan::queue` items
now says where the name comes from: `artisan command (built-in)`
for Laravel's own commands (`migrate`, `queue:work`, ...) and
`artisan command (project)` for the project's custom commands. The
offered names are unchanged - this is display provenance only, so a
static best-effort built-in list (item 409) is visibly distinct
from names scanned out of the project itself.

Provenance is decided by membership in the sorted
`BUILTIN_COMMANDS` list; a project command that reuses a built-in
name shows as built-in.

Tests: `offers_artisan_commands` pins the detail for both kinds of
name.
