# Livewire: Go to Handler

In a Livewire view (`resources/views/livewire/**.blade.php`), Go to Definition with the cursor on a member name opens the component class at that line:

- `wire:click="save"`, `wire:submit.prevent="save"`, `wire:model.live="form.name"` (opens `$form`), `wire:click="delete({{ $id }})"`. Attributes: click, submit, model, keydown, keyup, change, input, blur, focus, init, poll, intersect, mouseenter, mouseleave, dblclick, scroll, paste, select, confirm, with any modifiers.
- Alpine `$wire.save()` and `$wire.count` anywhere on the line.

The class is found from the view path (`livewire/admin/user-profile.blade.php` -> `app/Livewire/Admin/UserProfile.php`, or `app/Http/Livewire/...`). A method wins over a property of the same name; private and protected methods are skipped; properties must be declared `public` (or carry an attribute on the same line, such as `#[Locked] public int $count`).

Static text scan, nothing is run. `$refresh`, `$set(...)`, `$toggle(...)` have no PHP definition and are skipped. Members from traits or parent classes, Volt single-file components, and `wire:loading.class="..."` style values are not covered. Lines over 8 KB and class files over 1 MB are skipped; symlinks are not followed.

Status: unit tests cover attributes, `$wire.`, skips, symlinks and random-input fuzz. The GUI jump is not yet verified.
