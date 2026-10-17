# Laravel validation rule hover

In a PHP file, hovering a rule name inside a Laravel validation rule string shows one line on the rule from a built-in table, with any parameters echoed:

- `'title' => 'required|string|max:255',` - hovering `max` shows "at most the given value: string length, number, array count or file kilobytes (same as `size`)" plus "Parameter: `255`".
- Works for a pipe-separated list (`'sometimes|required|email:rfc,dns'`) or a lone rule as an array value (`'title' => 'required'`).

A string only counts when every `|`-separated rule name is known, and a lone word only counts as an array value (not a key), so ordinary text is left alone. Covers the Laravel 12.x validation rules (required, nullable, string, min/max/between/size, unique, exists, in/not_in, date rules, file/image rules, required_if family, and the rest). Descriptions follow the 12.x validation docs. Static text scan; nothing is executed. Lines over 4 KB are skipped.
