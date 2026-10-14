# Go to Policy / Gate

"Go to Policy / Gate" in the command palette lists Laravel authorization and opens the one you pick.

- `policy  PostPolicy  (viewAny, update, create)`: a class in `app/Policies`, opening the class line. Up to 10 abilities are shown.
- `ability  PostPolicy::update  Post`: one ability, opening its method. The trailing name is the type of the second parameter (the model).
- `gate  admin  (AuthServiceProvider)`: a `Gate::define('admin', ...)` call in `app/Providers`.

An ability is a public method other than `__construct`, `before` and `after`. A method with no visibility keyword counts as public.

Limits: static text scan, nothing is run. Policies registered by naming convention are listed whether or not they are registered. Gates defined outside `app/Providers`, and names that are not string literals, are not listed. Depth 3, 500 files, 1 MB per file, 5000 entries, symlinks skipped.
