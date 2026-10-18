# Go to Translation

"Go to Translation" in the command palette lists the project's translation keys and opens the line that defines the one you pick.

- `lang  auth.failed  (resources/lang/en/auth.php)`: keys from Laravel PHP group files (`resources/lang/<locale>/*.php`). Nested arrays are flattened with dot notation down to four levels, so `auth.throttle.seconds` and its parent `auth.throttle` both appear; picking one opens the file at that key's line.
- `lang  Welcome back  (en.json)`: keys from JSON translation files (`resources/lang/*.json`).

A key defined in several locales is listed once, for the alphabetically first locale. String escapes in keys are decoded (PHP string rules for `.php`, JSON rules for `.json`), so the key shown is exactly what `__('auth.failed')` or `trans('Welcome back')` expects.

Limits: static text scan, nothing is run. Only the modern `[...]` array syntax is parsed (the Laravel 9+ skeleton style); `array(...)` group files are skipped. Nested objects inside JSON files are not listed. Keys built dynamically and translations shipped by packages under `vendor/` are not listed. 1 MB per file, 20 locale directories, 100 PHP files, 5000 entries, symlinks skipped.
