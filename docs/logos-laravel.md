# Experimental Laravel conventions overlay (L5)

The existing `ProjectProfile.laravel` evidence and `logos-php` build flag select a separate overlay on PHP/HTML or Blade tokens. Default builds still use Lexilla. Symfony/WordPress-only profiles are unchanged. Detection itself is reused, not duplicated or replaced.

The overlay consumes byte styles from the base lexer, never reparses or executes PHP. It uses the editor's existing facade, route-method, Artisan, helper and Eloquent vocabularies from web metadata, plus conservative provider, middleware, jobs/events/listeners/notifications, policy/gate and container conventions.

- Listed facades require a static or namespace context.
- Route/Eloquent/relationship/container/convention methods require a member call.
- Helpers require a direct function call, excluding declarations and constructors.
- Standard base types and traits require a declaration, namespace or static context.
- Middleware aliases and Artisan command strings receive an overlay only inside matching calls.
- Comments, ordinary strings, variables and HTML retain base scopes.

This is lexical convention highlighting, not symbol resolution: a same-named user method may receive a convention scope. Custom aliases, inferred model types and semantic receiver tracking are not claimed. Original PHP/HTML IDs stay intact; overlay-only IDs40-44 are validated by a separate snapshot import. Regional PHP comment toggling understands these IDs.

Tests cover categories, opt-out, Blade host preservation, context leakage, UTF-8 prefixes, import validation and profile-driven integration. No new dependency or inference changes.
