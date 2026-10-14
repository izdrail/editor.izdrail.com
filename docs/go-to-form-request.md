# Go to Form Request

Command palette: **Go to Form Request** (`palette.form_requests`).

Lists every class under `app/Http/Requests` that extends a `...Request` class (normally `FormRequest`), and one `rule: field` entry for each top-level key of the array returned by `rules()`. Pick an entry to jump to the class or the rule line.

Static text scan only; no PHP is run. Nested keys (`'address' => ['street' => ...]`) are not listed, only the top-level field. Rules built from variables, `array_merge` or helper methods are not followed, so only keys written literally in `rules()` appear. Limits: depth 6, 1500 files, 1 MB per file, 100 fields per class, 5000 entries, symlinks not followed.

Status: unit tests cover parsing (brackets inside regex strings, nested arrays, multibyte keys, truncated files), limits and random-input fuzz. The palette GUI is not yet verified.
