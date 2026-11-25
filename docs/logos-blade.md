# Experimental Logos Blade mode (L4)

`gravity_lex::lex_blade` is a separate lexical entry point. `lex` remains PHP/HTML only. The existing `logos-php` build flag opts `.blade.php` into the new path; default builds continue to use Lexilla. Framework-specific PHP routes remain unchanged.

Coverage:
- Blade and custom directives, including balanced multiline PHP arguments.
- Escaped and raw echoes, nested PHP delimiters, strings, comments, heredocs and nowdocs.
- Blade comments, escaped directives/echoes and verbatim regions.
- `@php` blocks and `@php(...)` expressions.
- HTML host state across embedded expressions, including quoted attributes, x-components and Livewire namespaced tags.

No parser, template evaluation, PHP execution, I/O or new dependency. Existing PHP0-21 and host HTML32-36 style ABI is reused. Imported snapshots identify as `blade-logos`, separate from `php-logos`. This phase does not implement incremental reuse, Blade folding, framework semantic validation, or JavaScript/CSS embedded lexing.

Malformed/unfinished input is consumed to EOF without inventing parser recovery. ASCII directive identifiers follow Blade's lexical conventions. An email-like `name@domain.example` stays host text. Tests include UTF-8 prefixes and deterministic delimiter noise.

Verification: gravity-lex unit/contracts/mixed/PHP/Blade tests, lexilla-rs import tests, feature-gated lapce-core routing/scope/path test and the existing full hosted acceptance gates. Local lapce-core requires OpenSSL development headers, unavailable in this environment; hosted Ubuntu acceptance installs them.
