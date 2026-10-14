# Go to Laravel config and translation keys

"Go to Definition" in PHP and Blade files, with the cursor inside the key string:

- `config('app.name')`, `Config::get('mail.from.address')` (also `has`, `string`, `integer`,
  `boolean`, `array`, `float`): opens `config/app.php` at the `'name' =>` line. Nested keys are
  followed in order; if a deeper key is not found, the deepest one that is found is used, and
  `config('app')` opens the top of the file.
- `__('messages.welcome')`, `trans()`, `trans_choice()`, `@lang()`, `Lang::get()`: opens
  `lang/en/messages.php` (or `resources/lang/en/...`) at the key. The `en` locale is tried first,
  then the other locale folders.
- `__('Welcome back')` (text keys): opens the line in `lang/en.json` (then other `lang/*.json`).

Only the first argument counts (not the default value or replacements). Package keys
(`vendor::file.key`) are not resolved. Files over 1 MB are skipped; at most 40 locales are tried.
Keys defined by dynamic code, or config loaded from environment-only packages, are not found.
If nothing is found, normal Go to Definition runs. Nothing is executed.
