# Professional PHP/full-stack IDE roadmap

Baseline: `0127c7c`, 23 November 2026. This extends [the editing audit](jetbrains-editing-audit.md), not a restart. These are development tickets, not completed-feature claims. The 23:09 owner request sets the order below. No deployments are part of this work.

## Critical execution-order update, 23:16

The owner's newer autocomplete/formatting/AI settings task supersedes the earlier execution order below. **Strict gate: finish Phase 1 UI/UX audit, fixes and runtime acceptance before adding configuration or integrations.** The older feature tickets remain backlog, not concurrent permission to bypass this gate. Signature-help restoration work in progress was paused, not presented as complete.

| Phase / tickets | Scope and acceptance gate |
| --- | --- |
| 1 / UX-001..015, CFG-001..004 | Audit existing settings and workspace, fix navigation/organization and toolbar/context actions, standardize spacing/typography/icons/controls, remove confusing duplicates without silently changing saved routing, verify dark/light and responsive layouts. Inventory all visible controls and runtime states. Do not mark the phase complete from a few screenshots. |
| 2 / CMP-001..005 | Audit existing completion and reuse LSP/local providers. Configurable per-language/context single/multi-character triggers, scoped ranking/deduplication, manual/automatic invoke, debounce/limit, Enter/Tab/custom acceptance, closer integration, cancellation. Validate syntax examples rather than hardcoding universal triggers. Persist user/workspace settings for enabled/auto/triggers/providers/delay/limit/priority/case/inline/AI/shortcuts. Real-language/provider acceptance, not protocol fixtures, closes this phase. |
| 3 / FMT-001..004 | Audit existing formatter path. PHP-CS-Fixer/Pint/PSR-12/imports, Prettier/ESLint and Vue-aware formatting where supported. Document/selection/save/paste, per-language selection, detected project config and practical preview. Respect .editorconfig and formatter/linter files, never silently override project rules. |
| 4 / AGT-001..006 | Only after the earlier gates, Settings > AI & Agents: Providers, Coding Agents, Models, Authentication, Context/Permissions, Terminal, Behaviour, Usage/Logs. Investigate real Codex, Claude Code, Pi, Ollama and compatible-provider mechanisms/auth. No REST assumptions. Per-agent enable/name/provider/executable/model/endpoint/auth/workdir/env/context/permissions/default/status, configure/test/remove. Secrets backend-only; retain provider-specific capabilities. |
| 5 / QA-001..004 | Real runtime tests of settings persistence, triggers/acceptance/cancellation, project-respecting formatting, agent auth/execution/permissions, security and responsive themes. Unavailable tools remain PENDING, never simulated PASS. |

Phase 1 ticket detail: CFG-001 settings search/navigation and meaningful empty states; CFG-002 responsive spacing and accessible controls; CFG-003 existing AI settings duplication/routing clarity; CFG-004 toolbar/context-action inventory and runtime fixes. Phase 4's unified session interface covers select/start/context/progress/tool-permission/error/cancel only where the underlying real integration supports it. The standing development rule remains Ollama/local only, with no hosted AI calls.

## Search initiative update, 23:19

Search joins the backlog without weakening the critical strict phase order. Complete Phase 1 UI/UX first. Then run search architecture/benchmark work alongside Phase 2 completion, but prioritize completion acceptance over search feature expansion. Search UI fixes discovered during Phase 1 can land within that audit; a new indexing service cannot bypass it.

| Tickets | Search scope and gate |
| --- | --- |
| SRCH-001 | Audit native Rust app/proxy file access, unsaved buffers, existing watcher/index/cache and UI/RPC path. Evaluate ripgrep, Tantivy, Tree-sitter and SQLite FTS5 against existing dependencies. MiniSearch/workers are browser alternatives, not default infrastructure for this native editor. Licence review before adding dependencies. |
| SRCH-002 | Design a modular coordinator/providers/ranking/cache/history/indexer/storage/worker layer adapted to existing modules. Reuse existing providers and watcher. No duplicate service and no architectural decision without audit evidence. |
| SRCH-003 | Background incremental persistent index: mtime/hash, changed-only updates/deletes, in-memory unsaved overlays, async writes, restart validation/config-parser invalidation, cancellation/resumable recovery and low-noise progress. Never block UI or rebuild everything after one change. |
| SRCH-004 | Unified Search Everywhere, file/symbol/text/regex/usages/recents with scopes, filters, git awareness and previews. Semantic references remain distinct from text hits. Resolve double Shift and requested shortcuts against existing bindings; no silent shortcut regressions. |
| SRCH-005 | Rank exact filename, exact symbol, filename prefix, symbol prefix, word-boundary, partial, fuzzy, then text. Recency cannot displace better exact matches. 50-100ms debounce, obsolete cancellation, streaming, bounded concurrency, virtualized results and configurable ignore/binary exclusions. |
| SRCH-006 | Measure cold indexing separately; cached filename <20ms, symbol <50ms, indexed text/UI <100ms and typical small-file update <50ms are targets, not claims. Small/medium/large/thousands/large-file/frequent-edit fixtures, memory/concurrency/cancel/persistence/unsaved/delete tests. Report before/after measurements and misses. |
| SRCH-007 | Keyboard-first runtime UI acceptance of all dialogs/scopes/filters/previews/empty-error-loading states, focus and dark/light/responsive layouts. Report architecture decisions, changed files, real tests, measurements, limits and deployment requirements. No deploy. |

## Folding initiative update, 23:27

Folding UI defects join the existing Phase 1 audit. After that hard gate, audit/design/benchmark work can accompany Phase 2 completion and search, with completion acceptance first. Larger folding-engine changes wait for the architecture assessment and do not bypass the strict sequence.

Initial source assessment: this is native Rust, so use existing Rust crates directly, not WASM or another service. `lapce-core` already depends on Tree-sitter 0.22.6 and has parser/edit machinery. However, the active Lexilla path publishes Scintilla fold levels with an indentation fallback (`syntax/mod.rs`), not a structural Tree-sitter folding query engine. `lapce-core/src/folding.rs` owns line-based ranges/projection/state; `doc.rs` publishes accepted-revision ranges and maps edits. Fold state is document-shared and reconciles by header line, which does not meet requested stable structural identities or per-view state. Existing gutter, fold/unfold/all commands and shortcuts must be reused. Tree-sitter source presence does not mean active grammars or folding queries are available for every language.

| Tickets | Folding work and evidence gate |
| --- | --- |
| FOLD-001 | Deep architecture/grammar/licence audit and design before large changes: parsing, range generation/validation, state and UI boundaries. Study maintained open-source patterns without copying blindly. |
| FOLD-002 | Unified validated zero-based ranges with stable identity, positions, type, parent/children, provider, placeholder and language metadata. Tree-sitter primary; LSP/markers/indent/manual precedence and deduplication. Treat provider ranges as untrusted. |
| FOLD-003 | Validate separate per-language queries against actual grammars for PHP, JS/TS/JSX/TSX, Rust, Python, Go, HTML/CSS/JSON/YAML. No invented node names or brace-only primary engine. |
| FOLD-004 | Reuse InputEdit/incremental parser infrastructure with affected-range updates, stale cancellation, worker processing and delta publication. Stable reconciliation by syntax/context/fingerprint/parent/previous identity, not line alone. Paste/delete/rapid edit/undo/redo/multi-doc cases. |
| FOLD-005 | Independent view state plus safe per-file persistence, restart recovery, dead-range cleanup and edit expansion. Nested/parent/depth/recursive/others/toggle-all commands, meaningful bounded placeholders and existing shortcut conflict tests. |
| FOLD-006 | Gutter/hover/targets/position/focus/theme/no-layout-shift acceptance; dedicated persisted folding settings/defaults/per-language overrides only after settings architecture gate. No buffer corruption or copy/paste interference. |
| FOLD-007 | Measure targets: toggle/cache <5ms, incremental/restore <20ms, 1000-line parse <50ms, gutter <16ms. CPU/memory, malformed/large/deep/rapid fixtures, real provider tests and full editor regressions. These are targets, not achieved claims. |

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
