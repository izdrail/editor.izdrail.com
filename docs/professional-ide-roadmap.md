# Professional PHP/full-stack IDE roadmap

Baseline: `0127c7c`, 23 November 2026. This extends [the editing audit](jetbrains-editing-audit.md), not a restart. These are development tickets, not completed-feature claims. The 23:09 owner request sets the order below. No deployments are part of this work.

## Architecture audit and evidence

Gravity is a native Rust application built on Lapce and vendored Floem, not a Monaco/CodeMirror web application. `lapce-app` owns editor and panel state, `lapce-core` owns language/syntax support, `lapce-proxy` owns processes and services, `lapce-rpc` defines their protocol, and `lapce-ai` supplies existing AI support. Extend those boundaries rather than adding a parallel editor, parser, LSP client or service framework.

| Area | Existing source to extend | Acceptance boundary |
| --- | --- | --- |
| Editing | `vendor/floem/editor-core/src/editor.rs`, `lapce-app/src/doc.rs`, `lapce-core/src/pairing_context.rs` | Pairing, history and granular controls have hosted tests/UI captures. Current snapshot policy is conservative; Go has no Lexilla route. Structural bracket fallback is language-unaware. |
| LSP | `lapce-proxy/src/lsp.rs`, `lsp/registry.rs`, `plugin/mod.rs`, `lapce-app/src/editor.rs`, `completion.rs`, `hover.rs`, `rename.rs`, `code_action.rs` | Existing registry/stdio transport and editor UI must be reused. Definition mock-provider captures prove protocol routing only, not a real PHP provider. Signature-help RPC exists but editor update calls remain commented out. |
| PHP/frameworks | `project_completion.rs`, `class_refs.rs`, `php_imports.rs`, route/config/view indexes, `lapce-core/src/web_templates.rs` | Many bounded local helpers exist. They are not semantic type inference or a replacement for provider-backed PHP analysis. Verify each route with real projects. |
| Frontend | registry JS/TS/HTML/CSS/JSON entries; `web_templates.rs`, `vue_components.rs`, Tailwind modules | Source presence is not complete Vue/TS diagnostics, refactoring or build integration acceptance. |
| Tests | `lapce-app/src/test_runner.rs`, `test_index.rs`, terminal dispatch | PHPUnit/Pest command construction exists. Dedicated result trees, rerun failures, coverage and other runners need workflow audit. |
| Debugger | `lapce-app/src/debug.rs`, `lapce-proxy/src/dap.rs`, `dap/`, `php_debug.rs` | Real DAP and pinned Xdebug adapter preparation exist. Do not rebuild them or claim real Xdebug sessions from unit tests. |
| Formatters | `lapce-proxy/src/external_format.rs` | Existing opt-in Prettier/Pint/php-cs-fixer path has limits and timeouts. Real tools and errors need UI acceptance. |
| Git/terminal | `source_control.rs`, `worktree.rs`, `terminal/`, proxy terminal | Existing commands, diff/history and PTY should be extended, not replaced. Workspace trust and execution boundaries need review. |
| Database | No database execution workflow found in inspected command/app/proxy surfaces | SQL text highlighting and Laravel connection-name completion are not database management. Connection, secret and query execution work is pending. |
| AI | `lapce-ai`, app chat/inline modules and config | Existing local/Ollama paths and bounded review tools remain. Provider configuration is a roadmap item, not permission to call a hosted model during development. |

This is a repository-surface audit, not a claim every file was manually reviewed. Each ticket starts with a deeper audit of its source, data flow and dependencies. Existing docs and tests are leads, not proof that an integration is available now.

## Ticket order and acceptance

Status legend: OPEN means not accepted. AUDIT means inspect before defining the smallest implementation. Partial existing work does not close a ticket.

| Ticket | Priority / status | Work and required acceptance |
| --- | --- | --- |
| IDE-001 | P0 OPEN | Finish intelligent editing Phase 1: language-specific quoting, unsupported-language context, matching/unmatched feedback, large-file responsiveness. Exercise actual typing, multi-caret, paste, undo/redo, excerpts and settings restarts. |
| IDE-002 | P0 OPEN | Restore signature/parameter help through the existing RPC/provider UI, with revision/caret identity, cancellation and unsupported-provider state. Real provider invocation and popup UI required. |
| IDE-003 | P0 AUDIT | LSP capability matrix: completion/resolve, diagnostics, hover, definition/declaration, references/implementation, rename/code actions/imports. Verify real server sessions, stale responses and failure UI. No semantic claims based only on regex helpers. |
| IDE-004 | P0 OPEN | LSP security/performance audit: global-only command configuration, workspace trust, process lifecycle, bounded messages/context, cancellation, secret-safe diagnostics. Add adversarial integration cases. |
| IDE-010 | P1 AUDIT | PHP provider selection and licence review. Extend existing phpactor/Laravel registry support; evaluate Intelephense only with permitted licence/features. No new dependency before licence/provenance review. |
| IDE-011 | P1 OPEN | Real PHP 8.x project acceptance: PHPDoc/type inference, namespaces/imports, interfaces/traits/hierarchy, usages/refactoring/code generation where provider supports it. Mark missing capabilities pending. |
| IDE-012 | P1 AUDIT | Laravel capability matrix covering routes, Eloquent/relationships, migrations, Artisan, Blade/Livewire, container/facades, validation, events/jobs/policies. Keep bounded local navigation distinct from semantic provider results. |
| IDE-013 | P1 AUDIT | Symfony, WordPress and Composer/autoload paths. Extend existing modules and add representative project UI fixtures. |
| IDE-020 | P2 AUDIT | JS/TS/Vue provider intelligence, diagnostics/refactoring, component/prop/import navigation, HTML/CSS/JSON/YAML. Verify actual provider availability. |
| IDE-021 | P2 AUDIT | Tailwind suggestions, class tooling, npm/package tasks, existing formatter integration. Run real tools with bounded output and clear failure states. |
| IDE-030 | P3 AUDIT | PHPUnit/Pest dedicated results workflow: single/file/directory/suite, failure navigation, rerun failures, output and supported coverage. Add Behat/frontend runners only after provider audit. |
| IDE-031 | P3 AUDIT | PHPStan/Psalm/ESLint adapters: safe project opt-in, diagnostic mapping, cancellation and real error UI. |
| IDE-032 | P3 AUDIT | Existing DAP/Xdebug end-to-end: breakpoint/conditional breakpoint, step/continue/pause, variables/watch/call stack/exceptions/evaluate/console. Require a real PHP Xdebug session, not a simulated execution demo. |
| IDE-040 | P4 AUDIT | Git and terminal workflow/security regression: diff, history, staging/conflicts, remote operations and trusted execution. Preserve existing working routes. |
| IDE-041 | P4 OPEN | Database threat model/provider design before UI: proxy-mediated connections, encrypted secret storage, permission scope, no credentials in UI source/logs, explicit confirmation for destructive queries. |
| IDE-042 | P4 OPEN | Implement connection tests/schema/table/query/results/history/export/explain for SQLite then approved MySQL/PostgreSQL providers. MongoDB optional. Real database UI acceptance and cancellation/output bounds required. |
| IDE-050 | P5 AUDIT | Existing AI provider/config/context audit. Maintain local/Ollama development restriction, bounded relevant context, endpoint/key isolation, secret redaction and explicit apply/run boundaries. |
| IDE-051 | P5 OPEN | Extend explain/generate/refactor/docs/tests/exception/commit/project chat workflows only after auditing existing actions. No entire-repository upload by default. Real workflow and failure acceptance required. |
| IDE-060 | Cross-cutting OPEN | Finish live-template management using existing snippet infrastructure; keymap conflict tests, settings organization/persistence, accessibility/layout, documentation and outstanding-limitations register. |

## UI/UX audit, Addendum 16

The [UI/UX audit register](ui-ux-audit.md) tracks each surface and its verification gaps. UX-001 through UX-015 are cross-cutting work alongside the feature order above. Every visible control, keyboard path, state and backend result needs runtime verification. Most surfaces remain UNVERIFIED; existing screenshots are narrow evidence, not blanket PASS. Fix discovered problems, attach screenshots with severity/priority, and retain reasons/tickets for outstanding work. Native application/proxy logs and service failures replace browser-console checks where no browser surface exists.

## Completion contract

For each ticket: inspect current implementation, record scope/dependencies/licences, implement one focused change, run automated and integration tests, inspect actual UI pixels and workflow outcomes, cover failures/edge cases, review security, then update docs and limitations. Compilation and successful screenshots alone do not close a ticket. A deterministic protocol fixture must be labelled as such and cannot establish a real provider's behavior.

Use the existing main-only GitHub-hosted test/release/capture workflow. Keep local packaging/diff checks before pushes. Screenshots are hosted CI previews, not deployed releases. No new runner or host changes. Do not copy proprietary source or UI.

## Outstanding limitations at this baseline

- Four-phase editing initiative remains open. Separate settings exist, but dedicated organization and language overrides are incomplete.
- Missing/stale lexer context uses literal insertion; Go lacks a Lexilla route. Raw strings, lifetimes and complex interpolation need further policy and acceptance.
- Structural bracket fallback is language-unaware; unmatched feedback is not complete.
- Adjacent Match Pairs command code passes tests, but its first shortcut capture selected text instead of proving the intended jump. Workflow acceptance remains open until corrected and inspected.
- Signature/parameter-help UI restoration and real PHP/frontend provider matrices remain open.
- Deterministic LSP fixtures prove application routing, not phpactor/Intelephense/TypeScript provider quality.
- Real Xdebug, structured test results, database management and the requested static analyzers are not accepted by this audit.
- Broad performance, credential/logging and workspace-execution reviews remain required. Existing bounded helpers do not substitute for those reviews.
