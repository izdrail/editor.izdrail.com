# Find Component Usages

Command palette: **Find Component Usages** (`palette.component_usages`).

With a component file open, lists every line under `resources/views` that renders it. Pick one to jump there; the list filters as you type.

Recognised component files:
- `resources/views/components/**.blade.php` and `app/View/Components/**.php` -> `<x-name>`
- `app/Livewire/**`, `app/Http/Livewire/**`, `resources/views/livewire/**` -> `<livewire:name>` and `@livewire('name')`

Matching is whole-name: `<x-btn>` does not match `<x-btn-lg>`. The component's own view is skipped.

Static text scan only; no PHP is run. Limits: depth 8, 5000 files, 1 MB per file, 500 results, symlinks not followed. Only `*.blade.php` under `resources/views` is searched (not PHP classes, dynamic `<x-dynamic-component>` or string-built names). If the open file is not a component, the list is empty.

Status: unit tests cover name mapping, whole-name matching, limits and multibyte text. The palette GUI is not yet verified.
