# Storage disk completion (item 397)

`Storage::disk('s` and `Storage::drive('s` offer the filesystem
disks the project declares, from the `'disks' => [...]` array of
`config/filesystems.php` (Laravel 12.x `FilesystemManager`:
`disk($name = null)`, `drive($name = null)` as its alias).

`Storage::build(...)` is NOT a disk-name position - its non-array
argument is a local root path (Laravel 12.x line 106:
`build($config)` resolves an on-demand disk from an array, treating
a string as a root) - so it stays silent, as does `Storage::cloud()`
(no name argument).

Only the `disks` array's top-level keys are harvested: the
`'default'` key, per-disk settings one level deeper (`'driver'`,
`'bucket'`, ...) and commented-out disks invent nothing, comments are
masked first, and a file containing heredoc/nowdoc is skipped whole
(the item 390d guard). The receiver must be exactly `Storage::` -
`MyStorage::disk(...)` and `$storage->disk(...)` stay silent - and a
call written inside a comment or a string literal is not a call.

Tests: `detects_storage_disk_context`, `offers_storage_disks`.
