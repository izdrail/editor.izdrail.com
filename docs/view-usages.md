# Find View Usages

Command palette: **Find View Usages** (`palette.view_usages`).

With a Blade view open (`resources/views/users/show.blade.php`), lists every line that renders or includes it, and jumps there. This is the reverse of Go to Template, which goes from the call to the view.

A line is listed when it holds the view name as a whole quoted string (`'users.show'`) and a view call or directive: `view(`, `View::make(`, `Route::view(`, `->view(`, `->markdown(`, `@include` (and its variants), `@extends`, `@each`, `@component`, `@livewire`. A folder's `index.blade.php` is `users`, as Laravel names it.

Searched: `app/`, `routes/`, `packages/` and `resources/views/` (`.php` files; `vendor/` and `node_modules/` skipped). Component views (`components/`) and Livewire views (`livewire/`) are not treated as plain views here; use **Find Component Usages** for those if it is available. Not found: names built by concatenation or variables, `view()` with the name on a different line from the call, package views such as `pkg::name`, and views returned through helpers you wrote yourself. Commented-out calls are listed. Static text scan only; no PHP is run. Limits: depth 8, 3000 files, 1 MB per file, 500 results, symlinks not followed.

Status: unit tests cover name mapping, whole-name matching, discovery across PHP and Blade, vendor skipped, caps, symlinks and random-input fuzz. The palette GUI is not yet verified.
