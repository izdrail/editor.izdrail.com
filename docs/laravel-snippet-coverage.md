# Laravel snippet coverage checkpoint

25 November 2026. This maps the recorded A-J request to implemented examples and
remaining gaps. It is not full acceptance. Laravel170, Livewire17, Blade108.
The original supplied configuration attachment and the rest of the message after
SMS notifications were not available. The repository inventory is the audited
baseline; it is not claimed to be that missing attachment.

| Category | Implemented examples | Explicit remaining coverage or review |
| --- | --- | --- |
| A Routing | HTTP verbs, resource/API resource, groups, name/prefix/domain, binding, numeric/UUID constraints, fallback/redirect/view, limiter/throttle, invokable, signed/expiring URL | Review original route templates for imports and editable controller defaults; runtime fixture project not run |
| B Controllers/HTTP | Basic/base controller, invokable class, five-action API and seven-action HTML resource classes, typed request examples, JSON/redirect/download/file/stream responses, uploads, HTTP client | Scaffold methods remain explicit501, not working CRUD; runtime view/redirect and route binding fixtures pending |
| C Eloquent | Model, UUID/ULID, fillable/guarded, casts, accessor/mutator, requested relation families with return types, local/global scopes, observer method/registration/ObservedBy, creating/boot/booted, collection/pivot classes, soft deletes, factories, hidden attributes, allowlisted array/JSON serialization | Broader model-event coverage, serialization relationship/accessor behavior, runtime schema fixtures |
| D Database | Create/modify/drop migration, foreign/unique/composite indexes, transaction/raw/builder/typed Eloquent predicate group, chunk/lazy/pagination, seeder/factory, upsert/insert/key-bounded bulk update, named connection | Broader Eloquent Builder runtime fixtures; drop migration rollback intentionally throws until original schema supplied, backup/data restoration and runtime verification remain |
| E Validation | Inline/FormRequest/rule classes, nested arrays, file upload validation, custom messages, requiredIf condition, after hooks, explicit authorization placeholder | Further conditional/Rule-object examples and executable request tests |
| F Auth | Session login/logout, guard lookup, gates/policy, password hashing/reset-link/reset broker handler, verification routes, Sanctum scoped token/revocation | Custom guard and user-provider implementation, auth/authorization middleware examples and runtime security tests |
| G Middleware | Standard/terminable/parameter/alias/global append/response-header templates, method/boolean request inspection | Runtime ordering checks; input inspection does not replace validation/authorization |
| H Events | Event/listener/queued listener/subscriber/discovery/dispatch/ShouldBroadcast/ShouldBroadcastNow | Channel authorization and event-discovery runtime checks |
| I Queues | Job/unique/batchable, middleware, retry/backoff/failure hooks, chains/batches, after-response/after-commit/synchronous/conditional dispatch and queueable notification | Coverage review against every requested item; no live queue worker/transaction rollback test run |
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
