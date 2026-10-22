# Alpine event name completion (item 355)

Inside a tag, `x-on:` and `@` now offer the DOM event names (click,
submit, keydown, ..., labelled "Alpine event") while the event name is
being typed; after the dot the existing modifier completion is unchanged.
The context requires a real open tag (`<` after the last `>`), so a Blade
directive like `@foreach` or an `@` in text content is not affected.

Tests: `detects_alpine_event_context`, `offers_alpine_events`.
