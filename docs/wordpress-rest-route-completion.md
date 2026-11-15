# WordPress REST namespace and route completion

Typing inside `register_rest_route` offers the REST namespaces and
route paths the project itself registers (item 438).

## Where it works

- `register_rest_route('...', $route)` - the namespace is the first
  argument.
- `register_rest_route($ns, '...')` - the route path is the second.

## What is offered

Namespaces and route paths harvested from `register_rest_route`
calls across the project's `.php` files (skipping `.git`,
`node_modules` and `vendor`; at most 500 files, 1 MB each).
Textual scan; nothing is executed. Routes written with regex
capture groups (`/orders/(?P<id>\d+)`) fail the name charset and
are not listed - only plain literal paths complete. Names from
commented-out code, from lookalike functions and from inside
strings are not collected. Core namespaces (`wp/v2`, ...) are not
listed - only namespaces the project itself registers under.
Unsaved edits are not seen until the lists cache expires.
