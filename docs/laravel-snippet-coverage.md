# Laravel snippet coverage checkpoint

25 November 2026. This maps the recorded A-J request to implemented examples and
remaining gaps. It is not full acceptance. Laravel149, Livewire17, Blade108.
The original supplied configuration attachment and the rest of the message after
SMS notifications were not available. The repository inventory is the audited
baseline; it is not claimed to be that missing attachment.

| Category | Implemented examples | Explicit remaining coverage or review |
| --- | --- | --- |
| A Routing | HTTP verbs, resource/API resource, groups, name/prefix/domain, binding, numeric/UUID constraints, fallback/redirect/view, limiter/throttle, invokable, signed/expiring URL | Review original route templates for imports and editable controller defaults; runtime fixture project not run |
| B Controllers/HTTP | Basic controller, invokable class, five-action API resource class, typed request examples, JSON/redirect/download/file/stream responses, uploads, HTTP client | Seven-action HTML resource controller and base controller; scaffold methods are explicit501, not working CRUD |
| C Eloquent | Model, UUID/ULID, fillable, casts, accessor/mutator, requested relation families with return types, local/global scopes, observer method, creating/booted, collection/pivot classes, soft deletes, factories, hidden attributes | Explicit guarded configuration, boot method, broader model-event/observer registration, serialization patterns beyond hidden attributes, runtime schema fixtures |
| D Database | Create/modify migration, foreign/unique/composite indexes, transaction/raw/builder, chunk/lazy/pagination, seeder/factory, upsert/insert, named connection | Explicit drop-table migration, bulk update, Eloquent Builder-specific patterns, migration rollback/runtime verification |
| E Validation | Inline/FormRequest/rule classes, nested arrays, file upload validation, custom messages, after hooks, explicit authorization placeholder | Conditional validation and further Rule-object examples, executable request tests |
| F Auth | Session login/logout, guard lookup, gates/policy, password hashing/reset-link, verification routes, Sanctum scoped token/revocation | Custom guard and user-provider implementation, full password reset handler, auth/authorization middleware examples and runtime security tests |
| G Middleware | Standard/terminable/parameter/alias/response-header templates | Global registration and explicit request inspection patterns; runtime ordering checks |
| H Events | Event/listener/queued listener/subscriber/dispatch/ShouldBroadcast | Event discovery configuration and ShouldBroadcastNow example; channel authorization runtime checks |
| I Queues | Job/unique/batchable, middleware, retry/backoff/failure hooks, chains/batches, after-response and queueable notification | Explicit dispatch variants and coverage review against every requested item; no live queue worker run |
| J Notifications/mail | Notification, mail/database/Slack/Vonage methods, queued notification, mailable envelope/content/Markdown/attachment, queue/later | Package/channel integration fixtures, any omitted requirements beyond the truncated request |

## Quality gates and limits

- Pre-change prefix, description and body inventory is in snippet-audit-before.json.
- Laravel-family prefixes are uniformly lv- with uniqueness/parser tests. Other packs
  are deliberately unchanged. No old-prefix aliases were retained under that rule.
- New templates use qualified framework names or explicit imports. Placeholder
  defaults and PHP dollar/backslash preservation have expansion regressions.
- Policy rules now deny by default until application rules replace them. Controller
  scaffolds abort501 until implemented. These are deliberate safe insertion stubs,
  not production-complete application features.
- Full-class snippets preserve PHP open tags in empty files and avoid duplicating
  an existing opening tag in the accepted PHP insertion context.
- Automated parser/build success does not establish PHP/Laravel runtime correctness.
  No fixture Laravel application or all-template PHP lint pass has run here yet.
- Checklist entries are scoped examples, not a claim that every variation or
  application-specific import/schema/authorization requirement is solved.

Sources and per-slice notes are in laravel-snippet-audit.md. The complete generated
prefix reference is in usage/snippet-reference.md.
