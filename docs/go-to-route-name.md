# Go to a named route

"Go to Definition" with the cursor on the name in a route helper opens where the route is named:

- Laravel: `route('users.index')`, `to_route()`, `redirect()->route()`, `Route::has()`, `URL::route()`
  open the `->name('users.index')` (or `'as' => 'users.index'`) line in `routes/*.php`.
- Symfony: `path('blog_index')`, `url()`, `generateUrl()`, `redirectToRoute()` open the
  `#[Route(..., name: 'blog_index')]` (or `@Route(name="...")`) in `src/`, or the route key in
  `config/routes.yaml` / `config/routes/*.yaml`.

Works in PHP, Blade and Twig files. The name must match exactly. Not resolved: names built from a
class-level prefix (`#[Route('/blog', name: 'blog_')]` plus `name: 'index'`) and group name
prefixes. Implicit resource names such as `photos.index` or `posts.comments.show` open the
`Route::resource('photos', ...)` / `apiResource` / `singleton` line (an explicit `->name()` wins; `->only()`,
`->except()` and `->names()` are not checked). Limits: 1 MB per file, 60 route
files, 2000 controller files. If nothing is found, normal Go to Definition runs. Nothing is executed.

For the list of all routes, use "Go to Route" in the command palette.
