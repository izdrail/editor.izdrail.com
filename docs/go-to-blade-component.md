# Go to Blade Component

"Go to Blade Component" in the command palette lists every component in the project and opens the one you pick.

Sources, by file name only (nothing is read or run):
- `resources/views/components/**.blade.php` as `<x-forms.input>` (an `index.blade.php` names its folder)
- `app/View/Components/**.php` as `<x-forms.date-input>` (class names are kebab-cased)
- `app/Livewire/**.php` and `app/Http/Livewire/**.php` as `<livewire:admin.user-profile>`
- `resources/views/livewire/**.blade.php` for Livewire views with no class

A component with both a view and a class lists the view, as go-to on the tag does. For a Livewire component the class wins.

Limits: depth 8, 5000 entries, symlinks not followed. Package and vendor components, `Blade::component()` aliases and anonymous component paths registered in a service provider are not listed.
