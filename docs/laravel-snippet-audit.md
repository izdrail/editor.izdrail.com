# Laravel-family snippet audit

24 November2026. Existing inventory: Laravel41, Livewire17, Blade108. Complete pre-change prefix/description/template inventory is retained in snippet-audit-before.json. Other framework and JavaScript packs are outside this task.

Naming findings:10 Laravel prefixes lacked lv-, all Livewire/Blade prefixes lacked it. Contextually shared short prefixes overlapped with Twig. Normalize Laravel route prefixes, Livewire to lv-livewire-*, Blade to lv-blade-*. This is an intentional breaking prefix rename requested by the owner; old aliases are not retained because every prefix must start lv-. Templates preserved at this first stage.

Quality/coverage: existing Rust parser tests validate template syntax and key PHP dollar/backslash behavior, but do not establish Laravel runtime compatibility. A-J additions and import/return-type/template review remain open. Do not present normalization as the full expanded library.

Runtime Tab check found full-file templates duplicating an active PHP open tag. Completion now strips only the template's leading open tag in a pure-PHP insertion context, retaining all template code and placeholders. Empty-file templates retain their open tag. Namespace/import conflicts are not silently rewritten.

Routing/relations slice: Laravel grows from41 to56 entries. Added named/prefixed/subdomain/fallback/redirect/constrained/invokable/bound routes and typed through/polymorphic/one-of-many relations. Existing five relation snippets now carry fully qualified return types. New route snippets use a fully qualified Route facade; related model/controller classes remain editable placeholders. This is partial A/C coverage, not A-J completion.

API references checked: https://laravel.com/docs/12.x/routing and https://laravel.com/docs/12.x/eloquent-relationships .

HTTP/validation slice:15 more entries (Laravel71 total): JSON/redirect/file/
download/stream responses, explicit-timeout HTTP calls, validated private upload,
nested-array/custom-message validation, full FormRequest and ValidationRule
classes, authorization and after-validation methods. Policy/model/conditions
remain explicit placeholders, not silently permissive authorization defaults.
No automatic retry on non-idempotent POST. Added expansion checks for namespaces,
PHP variables and placeholder defaults. Full A-J coverage remains open.
Sources: https://laravel.com/docs/12.x/validation ,
https://laravel.com/docs/12.x/responses , https://laravel.com/docs/12.x/http-client .

Database/middleware slice:17 entries (Laravel88), covering transactions, bound
raw queries, chunk/lazy/pagination, upsert/insert/connections, reversible modify
migration, foreign/unique/composite constraints, terminable/parameter middleware,
bootstrap alias registration and response modification. Descriptions identify
context, version-specific registration and safety-sensitive choices.
Sources checked: https://laravel.com/docs/12.x/queries ,
https://laravel.com/docs/12.x/migrations , https://laravel.com/docs/12.x/database ,
https://laravel.com/docs/12.x/middleware .

Queue/event/auth/notification slice:17 entries (Laravel105), including unique
and batchable jobs, retries/failure hooks/middleware, chains/batches, queued
listeners/subscribers/broadcasts, gates/password hashing and queued/database
notifications. Descriptions state required queue/cache/channel/schema setup.
This remains partial F/H/I/J coverage, not full A-J acceptance.
Sources: https://laravel.com/docs/12.x/queues ,
https://laravel.com/docs/12.x/events , https://laravel.com/docs/12.x/authorization ,
https://laravel.com/docs/12.x/notifications .

Model/factory slice: ten entries (Laravel115), covering typed casts, enum
casts, soft deletes/UUID prerequisites, eager loading, hidden output attributes,
typed attribute transform/local scope, factory states and lifecycle callbacks.
Descriptions distinguish model members from casts-array entries and schema
requirements. Hidden attributes are not described as authorization. This remains
partial A-J coverage, not complete acceptance. Sources checked:
https://laravel.com/docs/12.x/eloquent ,
https://laravel.com/docs/12.x/eloquent-mutators ,
https://laravel.com/docs/12.x/eloquent-factories .

Mail slice: six entries (Laravel121), typed envelope/view/Markdown methods,
storage-disk attachment method and queued/delayed dispatch after transaction
commit. Descriptions state view, storage, authorization and worker prerequisites.
These are editor templates, no messages were sent. A-J still partial.
Source checked: https://laravel.com/docs/12.x/mail .

Auth slice: six entries (Laravel127), session login with regeneration, logout
with session invalidation/CSRF regeneration, explicit guard lookup, password
reset-link handler, limited-expiry scoped Sanctum token and user-owned selected
token revocation. Descriptions require configured guards/brokers, auth/CSRF
middleware, authorization and throttling as appropriate. These are insertion
templates, not complete secure authentication scaffolding. A-J remains partial.
Sources checked: https://laravel.com/docs/12.x/authentication ,
https://laravel.com/docs/12.x/passwords , https://laravel.com/docs/12.x/sanctum .

Routing follow-up: five entries (Laravel132), route-name groups, limiter
registration, throttled routes, signed routes and temporary signed URLs.
Descriptions require matching registration and separate authorization.
Sources: https://laravel.com/docs/12.x/routing , https://laravel.com/docs/12.x/urls .

Model follow-up: four entries (Laravel136), ULID trait with schema prerequisite,
custom pivot class, Eloquent collection class and CollectedBy model attribute.
Source checked: https://laravel.com/docs/12.x/eloquent ,
https://laravel.com/docs/12.x/eloquent-collections ,
https://laravel.com/docs/12.x/eloquent-relationships . A-J still partial.

Model lifecycle slice: four entries (Laravel140), booted method, creating callback,
named global scope registration and Scope class. Descriptions specify insertion
context and query-wide effects. Source APIs verified against Laravel12 framework:
https://raw.githubusercontent.com/laravel/framework/12.x/src/Illuminate/Database/Eloquent/Model.php ,
https://raw.githubusercontent.com/laravel/framework/12.x/src/Illuminate/Database/Eloquent/Concerns/HasEvents.php ,
https://raw.githubusercontent.com/laravel/framework/12.x/src/Illuminate/Database/Eloquent/Concerns/HasGlobalScopes.php ,
https://raw.githubusercontent.com/laravel/framework/12.x/src/Illuminate/Database/Eloquent/Scope.php .
A-J remains partial.

Controller slice: two class templates (Laravel142), single-action invokable and
five-action API resource controller. These intentionally return explicit501
until validation, authorization and application logic are supplied. No permissive
CRUD or invented validation rules. Seven-action HTML resource/base-controller
coverage remains open. Source: https://laravel.com/docs/12.x/controllers .

Notification channels: three methods (Laravel145), mail, current Slack
SlackMessage API and Vonage SMS. Descriptions state package, via(), credentials
and routing requirements. No service configured, dependency installed or
notification sent. Source: https://raw.githubusercontent.com/laravel/docs/12.x/notifications.md .
A-J still partial.

Email verification slice: four entries (Laravel149), signed/authenticated
verification handler, throttled resend POST, notice view and verified route.
Descriptions require MustVerifyEmail, configured mail and CSRF-protected web
routes. No email sent. Source: https://raw.githubusercontent.com/laravel/docs/12.x/verification.md .
A-J remains partial.

Existing-template corrections: lv-policy now returns editable false defaults
for view/update, fixing the empty bool method and removing an ungrounded user_id
ownership assumption. Both permissions must be implemented for the application.
lv-cast method parameters now fully qualify Eloquent Model instead of assuming
an import. Identity behavior is explicit; these methods alone are not a cast
class. Entry count unchanged. Expansion regressions added.

A-J acceptance checkpoint now tracked in laravel-snippet-coverage.md. Entry
counts alone are not completion. The checkpoint separates missing templates,
missing original attachment/tail and runtime validation not performed.

Conditional/global middleware slice: two entries (Laravel151), Rule::requiredIf
array entry with explicit editable condition and global append inside an existing
bootstrap callback. No replacement of the default middleware stack.
Sources: https://raw.githubusercontent.com/laravel/docs/12.x/validation.md ,
https://raw.githubusercontent.com/laravel/docs/12.x/middleware.md .

Event follow-up: discovery-path builder and synchronous ShouldBroadcastNow class
(Laravel153). Synchronous delivery is explicitly labelled as blocking. Sources:
https://raw.githubusercontent.com/laravel/docs/12.x/events.md ,
https://raw.githubusercontent.com/laravel/docs/12.x/broadcasting.md .
Runtime broadcaster/channel authorization remains unverified.

Database/model safety slice (Laravel156): explicit-key bulk query update rejects
an empty ID list and states Eloquent events are bypassed. Guarded model defaults
to all attributes, and boot override preserves parent boot for model traits.
Source for query update: https://raw.githubusercontent.com/laravel/docs/12.x/queries.md .
Model lifecycle source was verified in Laravel12 Model.php in the prior slice.
No database queries run; runtime fixtures remain pending.

Password reset broker handler (Laravel157): validates token/email/confirmed
password with Password::defaults(), delegates credential checks to broker, hashes
password, rotates remember token and emits PasswordReset after save. Description
requires guest/CSRF/throttle/broker setup. No real password or account changed.
Source: https://raw.githubusercontent.com/laravel/docs/12.x/passwords.md .

Controller follow-up (Laravel159): seven-action HTML resource and empty abstract
application base Controller. Resource actions remain explicit501 stubs; the
base template warns against overwriting an existing application class.
Source: https://laravel.com/docs/12.x/controllers . Runtime CRUD not claimed.

Resource binding correction: API/HTML scaffold model arguments now use an editable
post placeholder matching the default Post model, rather than the unrelated model
argument. Description requires matching the actual resource route parameter.
Source: https://raw.githubusercontent.com/laravel/docs/12.x/controllers.md

Observer registration follow-up (Laravel161): qualified model observe call for
a provider boot method and ObservedBy attribute for placement above a model class.
Expansion tests preserve class names and attribute punctuation. Observer business
logic and event dispatch remain application-specific and not runtime-verified.
Source: https://raw.githubusercontent.com/laravel/docs/12.x/eloquent.md

Drop/serialization follow-up (Laravel164): drop-table migration marks destruction
and refuses rollback until the developer supplies the original schema. It does
not pretend an empty table restores dropped data. Model array/JSON snippets clone
the instance and explicitly allowlist id by default, avoiding accidental broad
attribute/relationship exposure; developers must choose any additional fields.
JSON uses JSON_THROW_ON_ERROR. No migration or model runtime fixture claimed.
Sources: https://raw.githubusercontent.com/laravel/docs/12.x/migrations.md
https://raw.githubusercontent.com/laravel/docs/12.x/eloquent-serialization.md

Request/Builder follow-up (Laravel167): HTTP-method and boolean-input inspection
explicitly do not replace validation or authorization. A typed Eloquent Builder
closure groups a selectable model predicate. Expansion tests distinguish the
Eloquent Builder from the base Query Builder. No database/request runtime claimed.
Sources: https://raw.githubusercontent.com/laravel/docs/12.x/requests.md
https://raw.githubusercontent.com/laravel/docs/12.x/eloquent.md
https://raw.githubusercontent.com/laravel/framework/12.x/src/Illuminate/Database/Eloquent/Builder.php

Queue dispatch follow-up (Laravel170): after-commit, synchronous and conditional
dispatch templates. Descriptions distinguish current-process work from background
execution and condition checks from authorization. Expansion regressions cover
arguments and transaction method. No live queue/rollback acceptance claimed.
Source: https://raw.githubusercontent.com/laravel/docs/12.x/queues.md

Auth follow-up (Laravel173): request-guard registration scaffold denies all until
real credential verification is supplied, with explicit auth.php wiring note.
Route examples select a guard and pair authentication with policy middleware.
These do not implement a credential verifier or custom user provider, and no
runtime security acceptance is claimed.
Source: https://raw.githubusercontent.com/laravel/docs/12.x/authentication.md
