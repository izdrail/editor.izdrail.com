# Livewire: Find Member Usages

Command palette: **Livewire: Find Member Usages** (`palette.livewire_usages`). Run it with a Livewire component class (`app/Livewire/Admin/UserProfile.php`) or its view (`resources/views/livewire/admin/user-profile.blade.php`) active.

Each entry is one use in the view and opens it at that line:
`save  resources/views/livewire/admin/user-profile.blade.php:12  <button wire:click="save">`

- Counted: `wire:click`, `submit`, `model` (also `.live`, `.blur`, ...), `keydown`, `keyup`, `change`, `input`, `blur`, `focus`, `init`, `poll`, `intersect`, `mouseenter`, `mouseleave`, `dblclick`, `scroll`, `paste`, `select`, `confirm`; Alpine `$wire.name`. `wire:model="form.name"` counts as `form`.
- Only names the class declares (a non-private method or a `public` property) are listed, so `$refresh`, `wire:loading.class="..."` and misspelt handlers do not appear.
- The view is found by mapping the class path to kebab-case (`UserProfile` -> `user-profile`). If it does not exist, the palette is empty.

Static text scan, nothing is run. Limits: 1 MB per file, 8 KB per line, 500 entries, symlinks not followed. Other views that embed the component, traits and parent classes are not scanned.

Status: unit tests cover both entry points, skips, caps, symlinks and random-input fuzz. The palette GUI is not yet verified.
