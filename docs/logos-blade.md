# Experimental Logos Blade mode (L4)

`gravity_lex::lex_blade` is a separate lexical entry point. `lex` remains PHP/HTML only. Both the `logos-php` build feature and `GRAVITY_LEXER=logos` are required; default builds and feature-enabled builds without runtime opt-in continue to use Lexilla. See [the current guide](logos-lexer.md).

Coverage:
- Blade and custom directives, including balanced multiline PHP arguments.
- Escaped and raw echoes, nested PHP delimiters, strings, comments, heredocs and nowdocs.
- Blade comments, escaped directives/echoes and verbatim regions.
- `@php` blocks and `@php(...)` expressions.
- HTML host state across embedded expressions, including quoted attributes, x-components and Livewire namespaced tags.

No parser, template evaluation, PHP execution, I/O or new dependency. Existing PHP0-21 and host HTML32-36 style ABI is reused. Imported snapshots identify as `blade-logos`, separate from `php-logos`. Incremental reuse and deferred native folding are now provided by [L8](logos-incremental.md). Framework semantic validation and JavaScript/CSS embedded lexing are not provided by this scanner.

Malformed/unfinished input is consumed to EOF without inventing parser recovery. ASCII directive identifiers follow Blade's lexical conventions. An email-like `name@domain.example` stays host text. Tests include UTF-8 prefixes and deterministic delimiter noise.

Verification: gravity-lex unit/contracts/mixed/PHP/Blade tests, lexilla-rs import tests, feature-gated lapce-core routing/scope/path test and the existing full hosted acceptance gates. Core builds require native development prerequisites, including OpenSSL headers on Linux; hosted Ubuntu acceptance installs them.
