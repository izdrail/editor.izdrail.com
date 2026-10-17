# Laravel `Str::` / `Arr::` helper hover

In a PHP file, hovering the method name in `Str::slug($title)` or `Arr::get($array, 'a.b')` shows one line on the helper from a built-in table covering the `Str` (90 methods) and `Arr` (57 methods) helpers, grounded in the Laravel 12.x strings and helpers docs.

- `$slug = Str::slug($title);` - hovering `slug` shows "generates a URL friendly "slug" from the given string."
- The `Str::` or `Arr::` prefix is required and must be a whole word, so `MyStr::`, `$str::` and import (`use`) lines stay quiet. Bare function calls are not matched. Static text scan; nothing is executed. Lines over 4 KB are skipped.
