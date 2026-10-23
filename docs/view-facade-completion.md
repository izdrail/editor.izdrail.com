# View facade completion (item 396)

The `View` facade completes the same names as the `view('...')`
helper and the Blade view directives: `View::make('pages.ho` and
`View::exists('pages.ho` offer the project's views, and
`View::first(['emails.welcome', 'fall'])` completes each entry of the
fallback list.

The shapes follow Laravel 12.x `View\Factory`: `make($view, $data)`
and `exists($view)` take the name as a scalar first argument, while
`first(array $views)` takes a list - so a scalar `View::first('...')`
stays silent, and an array string in `make`/`exists` is data, not a
name. Only the outer list's entries complete; the data argument
(`View::make('pages.home', ['title' => ...])`) never does.

The receiver must be exactly `View::` - `MyView::make(...)` and
`$factory->make(...)` stay silent - and a call written inside a
comment or a string literal is not a call. Names come from the same
`resources/views` harvest as the helper.

Tests: `detects_view_facade_context`, `offers_view_facade_names`.
