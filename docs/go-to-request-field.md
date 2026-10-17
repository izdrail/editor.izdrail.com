# Go to a FormRequest rule from the field name

"Go to Definition" on the field name in `$request->input('title')`, `$request->has(...)`,
`$request->boolean(...)`, `$request->filled(...)`, `request('title')` or `old('title')` opens the
`'title' => ...` line of `rules()` in the current file, when the file declares
`function rules(` (a FormRequest). `request()->input('title')` works too; the receiver of the
`->input(...)`-style methods is not checked.

Covered methods: `input`, `query`, `post`, `string`, `boolean`, `integer`, `float`, `date`,
`has`, `missing`, `filled`, `exists`, `only`, `except`, `cookie`, `header`. Only simple keys
(letters, digits, `_`, `-`) are followed; dotted keys (`items.*.name`), variables and keys
without a matching rule do nothing, and normal Go to Definition runs. Static text scan only;
no PHP is run. Limits: 1 MB per file, 2000 lines scanned from `rules()`.

Status: unit tests cover the call shapes, both quote styles, the rule line lookup, negative
shapes (`myrequest(`, `$this->request(`, dotted keys) and non-ASCII fuzz. The GUI jump is not
yet verified.
