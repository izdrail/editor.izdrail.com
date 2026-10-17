# Go to Laravel gate

In a PHP or Blade file, put the cursor on the ability name and run Go to Definition:

| Use | Opens |
| --- | --- |
| `Gate::allows('x')`, `denies`, `check`, `authorize`, `inspect` | `Gate::define('x'` in `app/Providers` |
| `$user->can('x')`, `->cannot('x')`, `$this->authorize('x')` | same |
| `@can('x')`, `@cannot`, `@canany`, `@elsecan` | same |
| `->middleware('can:x,post')` | same |

Only names with a `Gate::define` are resolved, so policy abilities such as `can('update', $post)` fall
through to normal lookup (use "Go to Policy / Gate" for those). If a gate and a policy share a name, the
gate is opened. Arrays of abilities and names built at runtime are not resolved. Gates defined outside
`app/Providers` are not found. Limits: depth 3, 500 files, 1 MB per file, no symlinks. Nothing is executed.
