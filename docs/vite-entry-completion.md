# Vite entry completion (item 398)

`Vite::asset('resources/js/ap` and the `@vite` directive -
`@vite('resources/js/app.js')` or `@vite(['resources/css/app.css',
'resources/js/app.js'])` - offer the files under `resources/js` and
`resources/css` as root-relative paths, the strings Laravel 12.x
`Foundation\Vite::asset($asset)` and `Vite::__invoke($entrypoints)`
take.

Exact scope: `Vite::asset(...)` completes in any file (it is plain
PHP); the `@vite` directive completes only inside `*.blade.php`
files - in a plain PHP file `@vite(...)` is the error-suppression
operator on a `vite(...)` call and offers nothing, and an unknown
file type fails closed.

This is a different set from the `asset()` helper's: `asset()` /
`mix()` complete files under `public/`, Vite entry points are source
files, and the bare `asset('...')` context is untouched. The
build-directory second argument, a look-alike `MyVite::` receiver,
and calls written inside comments or strings stay silent.

The harvest is a bounded, symlink-skipping walk (the same
`walk_files` used for public assets), sorted and deduped.

Tests: `detects_vite_entry_context`, `offers_vite_entries`.
