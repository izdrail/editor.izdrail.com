# Go to Route (Laravel and Symfony)

Command palette: "Go to Route" (`palette.routes`) lists the routes declared in
`routes/*.php` (Laravel) and in `src/Controller` attributes and `config/routes.yaml`
(Symfony) of the workspace root, for example `GET /users  UserController@index`.
Choosing one opens the controller method when it can be found, otherwise the
`Route::` line. The list filters as you type, so a URI fragment or a controller
name both work.

Understood: `Route::get/post/put/patch/delete/options/any/match/view`, `resource`
and `apiResource` (expanded to their standard actions), `[Controller::class, 'm']`,
`'Controller@m'` and invokable `Controller::class` actions. Controllers are found
through the file's `use` lines (aliases included), or `App\Http\Controllers` for bare
names, and mapped to `app/` (PSR-4 `App\`).

Symfony: `#[Route('/x', methods: [...])]` on methods, with a class-level `#[Route]` prefix
applied, `path:` named arguments, and `path`/`controller` pairs in `config/routes.yaml`
(`App\\Controller\\X::method` or invokable). Attribute routes jump to the decorated
method; YAML routes jump to the controller method under `src/` when found. Docblock
`@Route` annotations, imported route files, `->name` and `{_locale}` prefixes are not read.

Limits: static text only, nothing is executed. Group prefixes, `->name()`, middleware,
and routes added by packages or loops are not tracked, so URIs are shown as written
in each call. At most 20 route files of 1 MB each and 5000 routes are read. WordPress
routing is not indexed.
