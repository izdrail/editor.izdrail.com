# Livewire: Go to Property / Action

Command palette: **Livewire: Go to Property / Action** (`palette.livewire_members`).

With a Livewire component open, from either its class (`app/Livewire/**`, `app/Http/Livewire/**`) or its view (`resources/views/livewire/**.blade.php`), lists the class's public members and jumps to the line:

- `property $name #Locked`: public properties, with any attributes on the line above.
- `action save()`: public methods, `#Computed` shown where present.
- `hook mount()`: `mount`, `boot`, `hydrate`, `dehydrate` and `updatedFoo` lifecycle methods.

Constructors and other `__magic` methods, and protected/private members, are left out. The view is mapped to its class by name (`admin/user-profile` -> `Admin/UserProfile.php`); if no class exists (a Volt or anonymous component) the list is empty. Static text scan only; no PHP is run. Limits: 1 MB class file, 300 members, symlinks not followed.

Status: unit tests cover visibility, attributes, braces in strings, the view-to-class mapping, limits and random-input fuzz. The palette GUI is not yet verified.
