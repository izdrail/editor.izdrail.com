# Bundled snippet review against target APIs

Item 448, 2026-11-15. Rank-9 plan gate: *final snippet code reviewed for
target API*. All 267 bundled snippets in `defaults/snippets/*.json` were
extracted and every API identifier (functions, classes, `use` statements,
attributes, Twig tags/filters/tests/functions, Blade directives) was
checked against pinned or current upstream sources. Result: **no snippet
needed a change**; the review did find and fix directive-name drift in
the Blade lexer list (below).

## Sources used

| Pack | Authority |
|---|---|
| wordpress.json (28) | `metadata/web/symbols.json` wordpress set (php-stubs/wordpress-stubs, source_commit per row) for every function; developer.wordpress.org reference pages for the classes the pack does not carry (WP_Query, WP_Widget, WP_Post, WP_REST_Request - all 200 OK) |
| twig.json (32) | `symbols.json` twig set (core tags/filters/tests/functions, pinned) plus the Symfony 7.4 pin `039017e8635dbc56d1f4bc930c60e5aa45d218e6` for twig-bridge registrations: RoutingExtension (path, url), AssetExtension (asset), CsrfExtension (csrf_token), FormExtension (form_start/form_widget/form_row/form_end), SecurityExtension (is_granted), TranslationExtension (trans filter) - all confirmed as `new TwigFunction`/`new TwigFilter` at the pin |
| symfony.json (23) | Symfony 7.4 pin raw files: AbstractController, WebTestCase, AsCommand, KernelEvents, Voter, AbstractAuthenticator, NormalizerInterface, AsMessageHandler, AsCronTask, AsEventListener, FormBuilderInterface, ConstraintValidator, Bridge/Twig TemplatedEmail; twigphp/Twig 3.x (AbstractExtension, TwigFilter); doctrine/orm 3.8.x (ORM\\Entity, ORM\\GeneratedValue, ORM\\Column attributes); doctrine/DoctrineBundle 3.4.x (ServiceEntityRepository); doctrine/persistence 4.2.x (ManagerRegistry); symfony/ux 3.x (AsTwigComponent, AsLiveComponent, LiveProp, DefaultActionTrait - all 200 OK) |
| laravel.json (41) | laravel/framework 11.x tree: every `Illuminate\\...` use statement fetched raw and confirmed 200 (Request, Model, Blueprint, Migration, Schema facade, Foundation\\Bus\\Dispatchable, InteractsWithQueue, Queueable, ShouldQueue, SerializesModels, Foundation\\Events\\Dispatchable, JsonResource, Console\\Command, MailMessage, Mailables\\Content, Mailables\\Envelope); helpers `view()`/`fake()` confirmed in Foundation/helpers.php; facades cross-checked with `LARAVEL_WORD_LISTS` |
| livewire.json (17) | livewire/livewire 3.x tree: Component, Form, WithPagination, attributes Rule, Validate, On, Computed, Locked, Modelable - all 200 OK. `#[Rule]` still ships in 3.x (Validate is the newer spelling); the snippet stays valid |
| blade.json (108) | Every directive cross-checked three ways: the vendored LexLaravel.cxx list, laravel/framework 11.x/12.x Blade compilers, and Livewire 3.x registrations. Details below |
| javascript.json (18) | Manual review: all 18 are stable ES2015+ syntax and DOM/Web APIs (fetch, Promise, addEventListener, querySelector(All), console.log, Array map/filter/reduce/forEach, class, import/export, try/catch) |

Method notes: method *declarations* inside snippet class bodies
(up/down/handle/rules/execute/supports/normalize...) are framework
contracts, not calls, and were adjudicated by hand. Pest's `it()` and
Laravel's `dd()` appear under their real names on purpose. The
`symbols.json` packs carry no framework classes for WordPress
(function-only), which is a pack gap, not a snippet error.

## Blade directive findings (fixed in this item)

The Blade snippet bodies were all correct. The review's three-way check
did find the vendored lexer's directive list had drifted from framework
truth; `LexLaravel.cxx` is now synced:

- **Removed `pushElseif`**: wrong name. The framework ships
  `compileElsePushIf` / `compileElsePush` (11.x and 12.x
  CompilesConditionals), i.e. `@elsePushIf` / `@elsePush` - both added.
- **Removed `elseunless`**: no `compileElseUnless` exists in 11.x or
  12.x.
- **Added** `elsePush`, `elsePushIf`, `prependOnce` (11.x/12.x
  CompilesStacks), `unset` (11.x/12.x CompilesRawPhp), `choice` (11.x/12.x
  CompilesTranslations), `hasStack` (12.x CompilesConditionals), `context`
  (12.x CompilesContexts; `@endcontext` covered by the end-form matcher),
  `entangle` (Livewire 3.x, repo tree evidence).
- **Verified present, kept**: `pushIf`, `elsePush`, `bool` (11.x/12.x
  CompilesConditionals - `pushIf` lives there, not in CompilesStacks; an
  intermediate draft of this review nearly removed it after searching the
  wrong file), `teleport` and `persist` (Livewire 3.x docs:
  livewire.laravel.com/docs/3.x/teleport and /docs/3.x/navigate),
  `livewire`/`livewireStyles`/`livewireScripts` (Livewire 3.x
  `Blade::directive` registrations in FrontendAssets.php),
  `viteReactRefresh` (12.x compile list), and all classic directives.
- **End-form casing**: `IsBladeEndDirective` now matches the directive
  part case-insensitively, so `@endPushOnce`, `@endPrependOnce`,
  `@endPushIf` and friends highlight; the old exact `strcmp` only
  recognised lowercase end forms.

Coverage test: `blade_directive_list_matches_framework_truth` in
`lapce-core/src/lexilla.rs` asserts every added/kept directive highlights
and that `@pushElseif`/`@elseunless` do not.
