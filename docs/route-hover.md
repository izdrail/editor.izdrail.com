# Laravel named route hover

In a PHP file, hovering the name in `route('users.index')`, `to_route(...)`, `routeIs(...)`, `Route::has(...)` or `URL::route(...)` shows the route's verb, URI and action from `routes/*.php`, with the file and line:

- `route('users.index')` shows "GET /users - UserController@index - routes/web.php:3".
- A name defined on the next line of a chained call (`Route::post(...)\n    ->name('users.store')`) still resolves to the enclosing call.
- Implicit resource names (`photos.edit`) show the resource declaration: "GET /photos (resource) - PhotoController (7 actions)".

Same callers, resolution and limits as the named-route go-to: names assembled from group prefixes are not resolved, the URI shows as written. Static text scan; nothing executed. 1 MB per routes file, 4 KB line cap.
