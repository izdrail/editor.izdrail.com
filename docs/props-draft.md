# Insert @props Draft

"Insert @props Draft" in the command palette, run in a component view (`resources/views/components/**.blade.php`), inserts an `@props([...])` block at the cursor. It lists the attribute names that other views pass to this component, most used first:

```blade
@props([
    'name',
    'label',
    'type',
])
```

- Names only: defaults cannot be known, so add them yourself (`'type' => 'text'`).
- `max-length` is written as `maxLength`, as Blade names the variable. `:disabled` and bare `required` count.
- Left out: `wire:*`, `x-*`, `@click`, `class` and `style`, and attributes with `:` or `.` in the name.
- Nothing happens if the file already has `@props`, or no view uses the component. A message says which.

Static text scan of `resources/views` (depth 8, 1500 files, 1 MB per file, 2000 bytes per tag, 30 names). Callers in PHP (`view()` data), Livewire classes and other folders are not scanned. Slot content is not an attribute and is never listed. Undo with the editor's undo.
