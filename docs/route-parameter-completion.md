# Route parameter completion (item 417)

The parameters array of `route('users.show', ['us` and
`to_route('users.show', ['us` offers the `{...}` placeholders of
the named route's URI (Laravel 12.x `helpers.php`:
`route($name, $parameters)` :868, `to_route($route, $parameters)`
:962). Optional `{item?}` and scoped `{post:slug}` placeholders
both name the parameter; a `->name('...')` on its own line still
pairs with its call (the route-hover enclosing-call rule), and
group-assembled names are not resolved (the route hover/go-to
limit).

A route without placeholders and an unknown or variable name offer
nothing. A positional list value has the same shape before the
cursor as an assoc key (the `=>` is not written yet) - the config
`getMany` arm accepts the same ambiguity. The first argument stays
with route-name completion; a scalar second argument, the third
argument and a call written inside a comment or a string stay
silent. An intervening unnamed route never lends its placeholders
to a named route, and two route calls on one physical line keep
their own URIs - the enclosing call is found by byte order, and a
line whose calls expand to a different number of parsed rows (a
resource) fails closed instead of borrowing.

Tests: `detects_route_param_context`, `offers_route_params`, plus
`name_replacement_kinds_are_php_file_only` pins.
