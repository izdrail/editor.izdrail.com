# Validation rule completion

Typing a Laravel validation rules string offers the framework's rule
names: `'req|mai` narrows to `required`, `email` and the rest of the
list as the editor filters by the word after the last `|`.

## Where it works (item 378)

- `$this->validate(['name' => '...'])` and
  `$request->validate(['name' => '...'])` - a value in the rules
  array, both the pipe form `'required|em'` and the nested array form
  `['required', 'em']`.
- `Validator::make($data, ['name' => '...'])` - the rules array is the
  second argument there.
- The Livewire `#[Validate('...')]` attribute - the rules string
  directly, or an entry of `#[Validate(['required', '...'])]`.
- `$validator->sometimes('reason', 'required|...')` - the conditional
  rules string is the second argument.

The rule list is a static pack: the Laravel 12.x validation
documentation's rule sections, plus `sometimes` (documented inline)
and `after_or_equal` / `before_or_equal` (in the docs category index
and implemented as `validateAfterOrEqual` / `validateBeforeOrEqual` in
`ValidatesAttributes`). 111 names, no parameters; fluent `Rule::`
builders are PHP code and stay out.

## What stays silent

Field names (the array keys), the custom-messages argument, rule
parameters (after `:` in the current segment, e.g. `max:10`), a bare
string argument to `validate(...)`, and calls unrelated to validation.
FormRequest `rules()` methods return a plain array with no call on the
line, so they are not detected. Quote escapes are not read.

Tests: `detects_validation_rule_context`, `offers_validation_rules`,
`validation_rules::tests::rules_are_sorted_unique_and_bare_names`.
