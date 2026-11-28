# JetBrains-inspired editing upgrade: architecture audit

Audit baseline: `3b889a8`, 23 November 2026. This is a source audit, not a claim of runtime acceptance or cross-language parity. The requested four phases remain open. Existing LSP services and rope-based editing stay in place.

## Expanded initiative, 23 November

The [professional IDE roadmap](professional-ide-roadmap.md) extends these four phases with prioritized PHP/full-stack development tickets and source-evidence boundaries. The [UI/UX audit](ui-ux-audit.md) inventories the required runtime surface checks and known gaps. Earlier baseline findings below are historical, not current absence claims: separate pairing/quote/skip/Smart Enter/indent controls and current-snapshot context guards have since landed; language-specific context and full workflow acceptance remain open.

## Already present, extend rather than duplicate

| Area | Evidence and current boundary |
| --- | --- |
| Bracket/quote insertion | `vendor/floem/editor-core/src/editor.rs`, `Action::insert_with_closer_indent`: parentheses, square/curly brackets, single/double quotes and backticks, nested insertion, selection wrapping, multiple carets and existing-closer skipping. `lapce-app/src/doc.rs` routes ordinary documents and source-mapped excerpts through this core. Needs stronger acceptance coverage and context policy. |
| Smart Enter | Core `insert_new_line` expands empty pairs, keeps line indentation and inserts one indent unit after unmatched openers. Closing-character insertion can resolve opener indentation through syntax. It is not a full language-specific indentation engine. |
| Indentation Backspace | Core `DeleteBackward` moves to preceding indent stops for spaces and deletes one tab. Pair deletion currently handles only a single region and can expand explicitly selected text, both need correction. |
| Matching and nesting | `lapce-app/src/editor/view.rs` paints enclosing bracket highlights and scope lines. Configuration includes bracket colorization. `lapce-core/src/syntax/mod.rs` resolves syntax pairs; `MatchPairs` and syntax-item selection commands exist in the editing core. Adjacent/unmatched behavior needs focused acceptance. |
| Tags/templates | `doc.rs` auto-closes web tags and Blade directives; `lapce-core/src/web_templates.rs` supplies context rules. Keep these separate from generic bracket handling and retain existing tests. |
| LSP assistance | `editor.rs`, `completion.rs`, `hover.rs`, `code_action.rs`, `rename.rs`, proxy plugin support: hover, completion metadata, code actions, rename, references, definitions, inlay hints. Availability depends on the active server; source presence does not prove every language's provider works. |
| Snippets | `snippet.rs`, `builtin_snippets.rs`, `defaults/snippets/`, `lapce-vsix`: bundled context-aware packs, imported VSIX snippets, numeric placeholders, next/previous placeholder commands. Do not add competing pack infrastructure. |
| Navigation | File/workspace symbol palettes, local/global jump history, declaration/references, document-symbol breadcrumbs and fold/unfold commands exist. |
| Settings/keymaps | `config/editor.rs`, `defaults/settings.toml`, settings UI and preset loader persist existing settings. `defaults/keymap-presets/phpstorm.toml` is a small compatibility subset, not complete JetBrains emulation. |

## Partial or missing

- Generic pairing is based on adjacent character classes, not current lexer string/comment regions. Escaped quotes, Rust lifetimes, Python quoting and mixed Blade/Vue contexts require explicit policies and tests. Do not infer the policy from file extension alone.
- Pair deletion is single-region only and does not track whether a closer was automatically inserted. Keep any initial correction conservative and document the lack of provenance tracking.
- Empty-pair Enter is generic; contextual language indentation, comment handling, whitespace-only pairs and multi-caret edge cases need coverage. Split indentation retention from structural expansion settings.
- Bracket highlights exist; dedicated unmatched feedback and clear adjacent-bracket acceptance are not yet established.
- Signature help has RPC/plugin plumbing but editor `update_signature` calls are commented out. `show-signature = true` does not establish working parameter information. Restore a revision/caret-bound UI rather than a second LSP client.
- Separate controls for quotes, brackets, closer skipping, Smart Enter and smart indentation are absent. Existing pairing/surround/tag/highlight/inlay/completion controls must remain compatible. A dedicated smart-editing settings group and language overrides need design.
- User/project template editing, dynamic variables and a management UI are not established by the bundled packs. Placeholder traversal exists; linked edits need an audit of repeated placeholder indices before claiming Live Templates support.
- Recent-workspace history exists, but a dedicated recent-files popup, bookmarks and TODO/FIXME navigation were not found in the inspected command/palette surfaces. Verify all command routes before adding any.
- Extract variable/method and import operations should expose applicable LSP actions, not promise language-independent refactorings.
- PhpStorm preset lacks many requested shortcuts. Resolve actual commands and conflict precedence first. Double Shift needs an input-level design, not a speculative binding.

## Implementation sequence

### Phase 1: core editing

1. Correct per-caret paired Backspace in the existing Floem action, preserving explicit range deletion and a single buffer transaction. Add all-six-pair, mixed multi-caret, whitespace, selection and undo/redo regression tests exercised by hosted gates.
2. Build an editing acceptance matrix around the same public `Action` API: insertion, skipping, wrapping, nesting, paste, undo/redo and tab/space indentation. Include real source language fixtures rather than claiming language coverage from generic tests.
3. Add a current-text lexical context policy through `doc.rs` and existing syntax/lexer paths. Keep multi-caret decisions per offset; conservative behavior where context is unknown; no redundant parser or document-wide lex per keystroke.
4. Add granular pairing/skip/smart-Enter controls with backward-compatible defaults, including projection-document routing. Add persistence and disabled-mode tests as each control lands, not only at the end.
5. Improve structural Enter, indentation Backspace and matching/unmatched feedback in focused changes. Verify actual editor pixels and keyboard behavior using synthetic captures, then representative real-project fixtures. No provider calls are required for core editing.

### Phase 2: assistance

6. Restore signature help end to end with request identity, document revision/caret checks, cancellation, keyboard invocation and popup acceptance. Reuse existing proxy and provider capability checks.
7. Audit hover, completion and intention/code-action UI, expose the relevant existing operations, and verify stale-response and unsupported-server behavior. Add missing functionality only after an observed gap.

### Phase 3: templates and navigation

8. Extend current snippet parsing/storage/completion for editable global/project templates, context filtering, linked placeholders and bounded dynamic variables. No arbitrary command execution for template variables.
9. Expose existing LSP refactors and navigation, then add confirmed missing recent-files, TODO/FIXME and bookmarks workflows. Reuse breadcrumbs/folding and existing history.

### Phase 4: integration and release acceptance

10. Finish settings organization and PhpStorm preset bindings with command resolution and conflict tests. Preserve other presets and modal editing.
11. Run the full hosted workspace regression plus focused editor-core tests and visual acceptance. Exercise Laravel/PHP, Blade/Livewire, Vue/TypeScript, Python, Rust and Go, and bound large-file/project responsiveness before declaring completion.

## Gates and constraints

- Existing repository, native Rust/Lapce/Floem architecture, no added dependencies unless justified.
- Six local packaging tests and diff checks before pushes; hosted focused and full workspace tests, release build and GUI capture before acceptance.
- Vendored core changes must be exercised by tests run from the app or an explicit core gate; the workspace suite alone must not be assumed to include dependency unit tests.
- Screenshot review is a separate gate from successful capture. Synthetic composer captures never send messages.
- No deployments or host changes. Existing GitHub-hosted workflow only. No hosted AI provider use.

## First Phase 1 change

Per-caret paired Backspace now removes an immediately adjacent empty pair at every caret in one transaction. Explicit selections delete exactly their selected range, and whitespace between delimiters is no longer consumed. No provenance tracking is claimed: adjacent manually typed empty pairs behave the same as automatically inserted ones. Four `lapce-core::smart_editing_tests` exercise the vendored action via the existing workspace, including all six pairs, mixed ranges/carets, Unicode and undo/redo. Hosted acceptance remains required.

## Smart Enter history correction

The expanded native-action matrix caught two independent undo groups for the two halves of an empty-pair Enter expansion. The core now inserts the complete expansion in one rope edit and places each caret on its inner line. Consecutive Enter actions remain separate undo steps. All-pair insertion/skipping/wrapping, nested insertion/paste, indentation stops and structural Enter across tabs/two/four spaces are explicit core gates. Hosted acceptance is required before accepting the correction.

## Adjacent lexical bracket matching, pending hosted acceptance

The adjacent-bracket slice reuses the exact-current lexer snapshot for code bracket jumps/highlights and ignores string/comment brackets. Typed nesting treats crossed pairs as Unknown, not proof that the original bracket is unmatched. Only reaching BOF/EOF with a nonempty stack yields Unmatched. A scan stops at 64 KiB, depth 256, unknown bracket scopes or embedded-language boundaries. These limits and stale/missing snapshots are Unknown, never evidence for red unmatched feedback. Truly unsupported lexers keep their existing structural navigation fallback without unmatched feedback. Enclosing-scope search from arbitrary code remains the existing implementation and is not claimed language-aware by this slice. No synchronous lexer or new parser is added.

Focused core tests cover PHP, Python, Rust, JavaScript, TypeScript and CSS, quoted/comment delimiters, adjacency, Unicode offsets, crossed pairs, stale revisions, length mismatch, unsupported scopes, embedded boundaries, and budgets. GUI fixtures verify exact native clipboard suffixes for code jumps and quoted-brace no-op, unchanged source, and unmatched feedback in dark/light. These tests are authored, not yet executed on hosted CI. HTML, Blade, Vue and Go adjacent matching are not accepted by this slice; Go has no Lexilla snapshot.

Review details: Doc memo retains one result keyed by buffer revision, syntax revision, cursor offset and Arc snapshot identity, so blinking/scrolling with unchanged evidence does not scan again. Snapshot identity also invalidates same-revision backend changes. `embed_at` linearly searches embedded regions on every scanned codepoint, so uncached cost is O(min(distance, 64KiB) times embed count); the hosted ignored timing test records the 64KiB single-line unmatched case with zero embedded regions. Measurement is pending, not invented. Pending async re-lex intentionally suppresses adjacent highlights until exact-current evidence returns; this conservative transient disappearance is accepted for this slice instead of stale highlights or false red. `Syntax::parse` assigns `rev` alongside `lexilla`, before publishing the current syntax to Doc, so this is the lexical snapshot revision as well as the syntax revision.

The hosted fixture uses the default Logos setting, not a Lexilla pin. Logos PHP/Blade styles are imported into the same `syntax.lexilla: Snapshot` interface through `from_mixed_php_styles` / `from_laravel_styles`; the field name does not imply a native-only backend. Native lexer fixtures remain in core tests. The imported operator positive controls prove that route without adding a second dependency to lapce-app. CSS source maps braces through SCE_CSS_OPERATOR (LexCSS.cxx operator transition and style5 tag); hosted tests must still prove it. Unmatched dark/light screenshots 14j/14k are manual pixel gates, not automatic color assertions. All paint_char_highlights callers were updated; LapceColor::LAPCE_ERROR exists; Snapshot APIs and Embed::Script exist; lapce-core already depends on lexilla-rs. HTML/Blade/Vue mixed-region acceptance remains open.

Default backend details: Logos is selected only for PHP/Blade (and the existing Symfony Twig path), not Rust/JavaScript/Python/CSS. Those languages fall through the same backend selector to native Lexilla snapshots, so `syntax.lexilla` is populated and adjacent matching uses them by default. Unsupported languages such as Go retain old raw behavior without red unmatched feedback. The feature-enabled test explicitly checks the Logos-imported PHP identity plus Matched, Ignored and stale Unknown results. CSS operator classification remains source-grounded but runtime-unverified until its test row passes. Timing runs with `cargo test --release`; it prints debug_assertions, per-scan time and a report-only target of less than 1ms/scan. A miss is reported rather than silently ignored. `Syntax::parse` sets self.rev=new_rev and self.lexilla=snapshot in the same result before publication; no separate tree-sitter revision bump is required for lexical eligibility.

### Hosted acceptance, 28 November 2026

- `477fc0d`, [run46115186116](https://github.com/izdrail/editor.izdrail.com/actions/runs/46115186116): exact cursor probes pass for accelerator opening jump, palette closing jump and accelerator return. Each probe replaces a distinct clipboard sentinel, checks first character and suffix, and verifies unchanged disk bytes. Selection pixels inspected.
- `cab181a`, [run46115695940](https://github.com/izdrail/editor.izdrail.com/actions/runs/46115695940): compile, focused lexical matching, Doc navigation decision table, full suites and GUI pass. Manual light-theme acceptance failed because14k remained dark. `a14c9b2`, [run46119640088](https://github.com/izdrail/editor.izdrail.com/actions/runs/46119640088), fixes the fixture; actual14j/14k dark/light red unmatched outlines were inspected. Both runs remain part of the evidence, not a retroactive light-theme pass on cab181a.
- `80c9fc9`, [run46123630077](https://github.com/izdrail/editor.izdrail.com/actions/runs/46123630077): all four jobs pass, including native-only bracket tests (6 passed), logos-enabled tests (7 passed), import tests, full workspace/providers/release and composer/chat GUI. Mixed snapshot tests cover HTML/Blade/Vue script/style local forward/back matches; string/comment braces; split same-kind and cross-kind regions; plain host text; every brace byte of well-formed/malformed echo delimiters; Blade raw/comment/escaped echoes and quoted text in @php/@if; Vue v-if quoted text. Exact routes logged: native Html/Vue=hypertext, Blade=blade; Logos request Blade=blade-logos, Html/Vue fall through hypertext. These are core snapshot tests, not GUI navigation acceptance for every mixed-language case.
- Blade Logos import remaps only host delimiter bytes carrying OPEN/CLOSE19/20 into private style37=keyword. PHP tag IDs19/20 retain operator scopes, ordinary expression operators remain11; incoming37 is rejected. Import tests pass. Actual14l/14m show echo/raw delimiters colored like @php keywords and neutral PHP tags in dark/light. No native Blade lexer change was made. The suspected malformed-echo no-red gap did not reproduce in the strict test matrix.
- Release64KiB single-line unmatched-opener timing on80c9fc9:430.3 microseconds per scan over100 iterations, zero embedded regions, debug assertions off. This meets the report-only target below1ms for that fixture, not a many-embed or whole-project performance claim. Earliercab181a measured653.553 microseconds. Memoization avoids repeated scans at unchanged revision/snapshot/caret.

Remaining scope: Go has no configured lexical snapshot; enclosing-scope matching from arbitrary code retains its previous scanner; mixed-language GUI navigation and representative real-project acceptance remain open. The four-phase editing initiative and broad UI/UX gate are not complete.

Mixed-language GUI navigation gate in progress: synthetic HTML/Blade/Vue files have distinct first-line markers checked by native copy before navigation. Script/style forward/back jumps and cross-script no-op use exact suffix assertions; first forward jump permits at most three exact-no-op settle attempts and logs WARN on a retried pass. The active-file-language screenshots require manual tab-title and status-language inspection, especially Blade rather than PHP. Until that pixel review, language routing and this GUI navigation gate are not accepted.
