# Laravel config / translation key hover

In a PHP file, hovering the key in `config('mail.from.address')` or `Config::get(...)` shows the value from `config/mail.php`, and hovering `__('messages.welcome')`, `trans(...)` or `@lang(...)` shows the translation from the first locale that has the key (`en` first):

- `config('mail.from.address')` shows `` `mail.from.address` = `'a@x.test'` `` with the file and line.
- `config('mail.from')` on a nested array shows `[ ... ] (nested array)` with the file and line, rather than dumping the array.
- `__('Welcome back')` shows the value from `lang/en.json`.

An unknown leaf key shows nothing (it never falls back to a parent array's value). Same callers, resolution and limits as the config/lang go-to: first entry wins, 1 MB per file, static text scan, nothing executed. Lines over 4 KB are skipped.
