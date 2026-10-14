# Go to a Blade component's prop

Go to Definition with the caret on an attribute name of `<x-...>` opens the component where that attribute is declared:

```blade
<x-forms.input type="email" :disabled="$off" max-length="20" />
```

- View component (`resources/views/components/forms/input.blade.php`): the line of the name inside `@props([...])`, as a key (`'type' => 'text'`) or a list item (`'type'`).
- Class component (`app/View/Components/Forms/Input.php`): the constructor parameter line (`public string $type`).
- `max-length` matches `maxLength`, as Blade names the variable. A leading `:` or `::` is ignored.
- The tag may span lines. The scan looks back up to 30 lines and 16 KB.

Not handled: `wire:*`, `x-*`, `@click`, `class`/`style` merged attributes, attributes the component does not declare (go-to falls through to the normal behaviour), anonymous components with `@props` built dynamically, and props passed through `@aware`. Caret on the tag name still opens the component file as before.

Static text scan only. Nothing is executed. Component files over 1 MB and symlinks are ignored.
