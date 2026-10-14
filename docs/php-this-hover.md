# `$this->member` hover (fallback)

In a PHP file, hovering `name` in `$this->name` or `$this?->name()` shows the declaration from the same file when the language server has no hover for it (none installed, or it returns nothing). A language server answer always wins.

- Property: `public string $email = '';` with any `#[Computed]`, `#[Url]`, `#[Locked]` attribute lines directly above, and its line number.
- Method: `public function save(): void`.
- Eloquent model lists: if the name is in `$fillable`, `$guarded`, `$hidden`, `$visible`, `$appends`, `$dates` or `$with`, or in `$casts` (shown with its cast, for example `'datetime'`).
- Promoted constructor parameters are not matched as properties.

Limits: only the current file, so inherited members and trait members are not found. The model list notes appear even when the property itself is not declared (columns are not declared in the class). Static text scan, 1 MB and 8 KB-per-line limits.
