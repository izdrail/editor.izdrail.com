# Experimental Logos highlighting

Lexilla remains the default. Logos support is opt-in at build time **and**
runtime. The [benchmark gate](logos-benchmarks.md) found no basis for changing
the default. This guide describes the current implementation; the
[initial plan](logos-lexer-plan.md) and numbered phase notes contain historical
checkpoints, not current feature guarantees.

## Enable, disable and verify

From the repository root:

```sh
cargo build --locked --bin gravity --features lapce-app/logos-php
GRAVITY_LEXER=logos target/debug/gravity
```

Restart the process to change the environment. Only the exact value `logos`,
case-insensitive, enables experimental routing. An unset, empty, unknown or
`lexilla` value uses native routing. A build without the feature stays native
even with the environment variable. No persistent setting is written. Disable
by restarting without `GRAVITY_LEXER`, or use `GRAVITY_LEXER=lexilla`.

Tests inspect snapshot identity rather than infer the backend from similar
colours. `php-logos`, `blade-logos`, `laravel-logos`, `blade-laravel-logos` and
`symfony-logos` identify imported experimental snapshots. Symfony Twig is an
existing template composition, not the new PHP/Blade scanner.

| Context | Opt-in behaviour |
|---|---|
| Plain PHP, including mixed HTML/PHP | PHP/HTML driver |
| Blade, outside a Laravel project | Blade driver, no Laravel overlay |
| Laravel PHP or Blade | Base driver plus Laravel convention overlay |
| Symfony PHP, without Laravel evidence | Base driver plus Symfony overlay |
| Symfony Twig | Existing Twig composition plus bridge vocabulary |
| WordPress-only PHP or an unsupported language | Existing native/composed path |

Existing `ProjectProfile` detection owns project evidence. Nearest Composer
roots are boundaries; metadata selection is reused. Laravel has precedence
when both framework profiles apply. This is not runtime framework discovery.

## Architecture and ownership

1. `gravity-lex/src/lib.rs` owns Logos token enums and a hand-written state
   driver for PHP/HTML and Blade. Input is UTF-8 text. `Output` contains one
   `u8` style per **byte**, transition checkpoints and safe line checkpoints.
   Tokens never execute PHP, read files or access editor state.
2. `gravity-lex/src/laravel.rs` and `symfony.rs` apply optional convention
   overlays using supplied vocabulary. Profile detection and versioned metadata
   stay in `lapce-core`, outside the lexer crate.
3. `lexilla-rs/src/lib.rs` validates byte length and allowed style IDs on import.
   Native PHP IDs 0-21 remain intact; HTML host IDs 32-36 and framework IDs
   40-47 have explicit scope mappings. Themes consume the existing snapshot
   interface. PHP comment context also understands these IDs.
4. `lapce-core/src/lexilla.rs` owns the single backend selection hook, profile
   routing, snapshot import and native fallback. The public non-incremental
   highlighting path preserves native folds in the same request. The document
   incremental path separates colour results from deferred native folds.
5. `lapce-core/src/syntax/mod.rs` and `lapce-app/src/doc.rs` reuse the background
   parse worker, revision gate and cancellation. An Arc carries cache state;
   the UI callback does not scan the document. Native fold refresh is a separate
   250 ms-debounced background job; only same-revision fold ranges are accepted.

`incremental::Cache` reuses an unchanged prefix and restarts at a safe line or
multiline opener. A 64 KiB invalidation allowance handles small suffix work.
Larger invalidations use `cooperative::Session`, preserving the scan stack
across 16 KiB work permits. Cancellation discards unfinished results. This is
not a promise of sublinear diff/copy/allocation or a wall-clock time bound.
See [incremental design and limits](logos-incremental.md).

Completion, formatting, diagnostics, navigation, inference and serialization
remain separate systems. A lexer is not a parser or a language server.

## Supported lexical syntax

- PHP open/echo/close tags and transitions into HTML, with tag-boundary checks.
- Comments and PHPDoc, variables and Unicode identifiers, operators, numeric
  spellings, keywords, built-in types and ordinary names.
- Single/double/backtick strings, escapes, simple and balanced braced
  interpolation, heredoc/nowdoc with indented labels and CRLF.
- Modern declaration vocabulary, attributes, namespaces, union/intersection
  type punctuation, enums, readonly and PHP 8.4 property-hook/asymmetric
  visibility spellings. These are lexical scopes, not grammar validation.
- HTML tags, attributes, quoted values and comments. PHP islands preserve host
  context, including islands inside an HTML attribute or comment.
- Blade directives and custom directive names, balanced arguments, escaped/raw
  echoes, comments, verbatim regions, `@php` blocks/expressions, x-components and
  Livewire-style names. See [Blade coverage](logos-blade.md).
- [Laravel conventions](logos-laravel.md): facade/helper/member-call context,
  providers, middleware, jobs/events, policy/gate and container shapes.
- [Symfony conventions](logos-symfony.md): controller/DI/console/event/Messenger,
  forms/validation and Doctrine types/attributes; Twig bridge vocabulary.

## Add or extend a framework

1. Reuse `ProjectProfile` and `web_metadata` evidence where possible. Keep
   Composer/version selection and filesystem reads out of `gravity-lex`.
2. Add a separate overlay module consuming source plus validated base byte
   styles and caller-supplied vocabulary. Do not replace PHP string/comment/host
   scanning with framework regexes. Preserve ordinary text outside explicit
   lexical contexts. Yield/check cancellation in long loops.
3. Allocate unused IDs without changing the PHP 0-21 contract. Add an explicit
   validating snapshot importer and scope mapping in `lexilla-rs`; reject wrong
   lengths and unsupported IDs. Update PHP comment-context classification when
   a new PHP-owned ID needs it.
4. Route the overlay only through the existing opt-in hook with explicit profile
   precedence and native fallback. Keep default routing and native fold refresh.
   Do not add a second environment selector or change inference/serialization.
5. Test positives, same-name false positives, comments/strings/HTML exclusions,
   UTF-8 prefixes, malformed/incomplete input, snapshot IDs/scopes, profile
   opt-out, incremental/full equivalence, cancellation and stale revisions.
   Add representative fixture data and measure the production overlay cost.
6. Document lexical ambiguity and metadata/version limits. A name matching a
   framework method does not establish receiver type, alias resolution or
   semantic correctness. A new framework must not silently become default.

## Validation and performance

```sh
cargo test --locked -p gravity-lex
cargo test --locked --manifest-path lexilla-rs/Cargo.toml
cargo test --locked -p lapce-core --features logos-php
bash scripts/benchmark-logos.sh
```

Install the platform's native development prerequisites for the editor before
core/app builds. The [corpus gate](logos-corpus.md) uses PHP `token_get_all`
without parsing/evaluating fixture text. PHP is required in CI; a missing local
CLI is reported as an explicit oracle skip. Unit checks alone are not equivalent
to an oracle run. Hosted acceptance also builds/tests feature-enabled app code,
runs the default workspace, and checks the existing GUI/build acceptance path.

Release results and reproduction are in [logos-benchmarks.md](logos-benchmarks.md).
Initial and edit times, isolated retained RSS and worker/cancellation proxies
are separate measurements. None measures actual input-to-paint/frame latency.
Native-scope agreement is not an accuracy percentage. Keep the default gate
closed until complete-workload results justify a change.

## Known limits

- No AST, semantic receiver/type inference, alias resolution, PHP diagnostics,
  PHPDoc type parsing or exhaustive PHP grammar validation.
- Ambiguous short `<?` tags are not enabled. Project `short_open_tag` settings
  are not interpreted. Malformed input is styled conservatively through EOF.
- Whole braced interpolation uses the shared interpolation scope rather than
  semantically styling every nested expression.
- Experimental PHP/Blade does not add native-equivalent embedded JS/CSS raw-text
  overlays. HTML styling is not DOM parsing or validation.
- Safe restart may go back to a multiline opener; large edits can require a full
  scan. Framework overlays still inspect the full token list.
- Native folds remain a separate, uninterruptible native call. Previous
  edit-tracked folds stay until a same-revision refresh arrives.
- Work permits bound scan work, not OS scheduling/allocation or a real-time
  deadline. Finite tests do not prove universal termination or panic-freedom.
- Performance varies by input: tail reuse helps some cases, while initial and
  framework-overlay costs can be worse. No default switch or speed guarantee.
