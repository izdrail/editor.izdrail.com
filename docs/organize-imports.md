# Organize PHP Imports

Command palette: **Organize PHP Imports** (`organize_php_imports`, no default key). In a `.php` file it removes unused `use` lines and sorts the rest alphabetically (case-insensitive, like Pint's default `ordered_imports`). It is one undo step. A toast says how many imports were removed, or that they were already organized.

- Each run of consecutive `use Foo\Bar;` / `use Foo\Bar as Baz;` lines is cleaned on its own, so your blank-line groups stay as they are. Duplicates are dropped.
- An import counts as used when its short name (or alias) appears as a word anywhere else in the file, in code, strings or docblocks, case-insensitively. This errs towards keeping an import. For example a class named only in a string stays.
- Left untouched: group imports (`use A\{B, C};`), `use function`, `use const`, trait `use` inside classes, and anything below the first class/function declaration.
- Not run on `.blade.php` files, or on files over 1 MB.

Not covered: PHP attributes and `@see` references are plain text matches like everything else. A name that only appears inside another qualified name (`\Foo\Bar\Baz` for import `Baz`) counts as used, so it is kept.
