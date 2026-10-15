# Go to Inertia page

Go to Definition (the usual key or Ctrl+click) with the cursor on the page name opens the component:

- `Inertia::render('Users/Index', ...)`, `\Inertia\Inertia::render(...)`, `inertia('Users/Index')`
- `Route::inertia('/users', 'Users/Index')` (the second argument; the URL is not a page)

Looked up in `resources/js/Pages` then `resources/js/pages` (the Laravel starter kits use lowercase), with extensions `.vue`, `.tsx`, `.jsx`, `.svelte`, `.ts`, `.js`, in that order.

Textual only: nothing is executed. Page names with `..`, a leading `/`, `Module::Page` prefixes or backslashes are refused, symlinked files are not followed, and custom page paths set in `app.js` are not read. The reverse direction (from a page to the controller) is not covered.

Status: unit tests cover the call forms, missing pages, traversal, symlinks and random-input fuzz. The editor GUI jump is not yet verified.
