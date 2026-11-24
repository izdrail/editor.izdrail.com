# UI/UX audit and runtime verification register

Baseline: 23 November 2026, main `0127c7c`. This is an OPEN audit, not a declaration that every control has been tested. It accompanies [the IDE roadmap](professional-ide-roadmap.md) and the existing editing audit. All current captures are hosted CI previews, not deployed/live-product evidence.

## Critical Phase 1 gate

The 23:16 request makes this audit and its fixes the first execution phase. Completion, formatting and new AI agent settings wait for UI/UX acceptance. CFG-001..004 in the roadmap track settings navigation/search, responsive accessible controls, routing/duplicate clarity and toolbar/context actions.

## Verification contract

Inventory every visible control in each screen. Record its command/handler and expected state change, test pointer and keyboard paths at runtime, inspect actual pixels, and check application/proxy logs and any failed service request. Cover loading, error, empty and success states, disabled controls and unavailable integrations. Check narrow/wide layouts, scrolling/overflow, panel resizing/repositioning, dark/light themes, focus order, labels/tooltips and keyboard accessibility. An unchecked control stays UNVERIFIED, never PASS.

Each issue needs an ID, severity/priority, reproducible workflow, observed result, expected result, screenshot/run reference, fix or larger ticket, and remaining reason. Fix straightforward issues in the same workflow where possible. Larger fixes stay explicitly open. No mock-provider result is evidence of a working real provider, and source inspection is not runtime verification.

## Surface inventory and ticket register

| Ticket | Surface | Status and required runtime checks |
| --- | --- | --- |
| UX-001 | Workspace, title/toolbars, status bar | UNVERIFIED inventory. Every button, command target, tooltip, focus path, empty workspace and narrow/wide layout. |
| UX-002 | Explorer/file navigation | PARTIAL file-palette and Ctrl-click evidence only. Verify tree expand/collapse, create/rename/delete, context menus, errors, long names, scroll and permissions. |
| UX-003 | Tabs/splits | UNVERIFIED complete control inventory. Dirty state, close/cancel/save, overflow, drag/reorder, split focus and resize; no data loss. |
| UX-004 | Palette and keymaps | PARTIAL palette file/command workflows. Verify list focus, filtering, cancellation, unavailable commands and all preset conflicts. Match Pairs shortcut capture currently selects rather than proving the jump, see UI-001. |
| UX-005 | Search/replace | UNVERIFIED full workflow. Selection/current file/project modes, regex errors, next/previous, preview/apply, cancellation and large output. |
| UX-006 | Terminal/output/problems | UNVERIFIED full workflow. Real PTY/process, tabs/close/kill, resize, scroll, focus, links and failure/cancellation states. |
| UX-007 | Debug panels | UNVERIFIED real-session controls. Connect to actual DAP/Xdebug; step/pause/watch/variables/stack/console and disconnected/error state. |
| UX-008 | Testing panels | UNVERIFIED structured workflow. Existing terminal test commands are not evidence of a dedicated results panel. Follow IDE-030. |
| UX-009 | Database interface | PENDING integration/design. Do not present a decorative database panel as functional. Follow IDE-041/042. |
| UX-010 | Git/history/diff | UNVERIFIED complete runtime inventory. Staging, conflicts, branches, remote failures, confirmation and keyboard flow. |
| UX-011 | AI assistant | PARTIAL composer navigation and review evidence. Complete control inventory, narrow layouts, scrolling, menus/tooltips, loading/error/cancel, model/provider state and safe apply/run boundaries. Inspect small-panel clipping rather than assuming layouts are complete. |
| UX-012 | Settings/preferences | PARTIAL live smart-editing setting reload and compatibility tests. Verify actual settings controls, persistence after restart, filtering, validation, long descriptions, themes and shortcuts. |
| UX-013 | Menus/dialogs/dropdowns | UNVERIFIED global inventory. Correct targets, escape/outside-click behavior, focus return, disabled states and screen-edge placement. |
| UX-014 | Accessibility/themes/layout | UNVERIFIED cross-surface audit. Keyboard reachability, visible focus, labels, contrast in both themes, zoom, overflow and responsive sizes. |
| UX-015 | Runtime diagnostics | UNVERIFIED baseline. Review application/proxy errors and failed network/backend calls per tested workflow; known empty-provider errors must be classified rather than hidden. |

## Identified issues and evidence

| Issue | Severity / priority | Evidence and disposition |
| --- | --- | --- |
| UI-008 | Medium / P0 | Source audit: generic boolean setting targets and several existing AI/model actions were omitted from keyboard traversal, and shared action styling lacked a visible focus state. Fix in progress: use existing framework keyboard activation and focus treatment, preserve handlers, and verify a real settings toggle persists through keyboard input. No model download or remote inference is started for acceptance. |
| UI-007 | Medium / P0 | Source audit: settings search retained every section header even when no setting matched, leaving an unexplained list of empty headings. Fix in progress: retain only sections with matches, searchable section labels, explicit zero-results state; actual dark/light/narrow runtime acceptance pending. |
| UI-001 | Medium / P0 | Run `45283575269`, `14d-bracket-jump-from-adjacent.png`: intended Ctrl+Shift+backslash action leaves six characters selected, not the asserted jump to opening brace. Unit tests pass, but this runtime workflow is NOT ACCEPTED. Inspect preset/input mapping and retry via the existing command palette before deciding whether implementation or shortcut is wrong. |
| UI-002 | Medium / P0 | Earlier Ctrl-click rapid-click/focus dispatch hazards and cross-file equal-coordinate LSP self-detection were fixed. Hosted workflow captures verify exact paths, edited source and deterministic LSP routing; the user's exact failing project/provider remains unverified. Do not close every Ctrl-click scenario from synthetic evidence. |
| UI-003 | Medium / P0 | Bracket highlights previously failed when current syntax text existed without tree layers. Fixed and runtime-verified in `45275627813`, images `14`, `14a`, `14b`; after-closer highlight verified in `45279064490`, `14c`. Language-aware matching and unmatched feedback remain open. |
| UI-004 | Medium / P0 | PHP comment-end partner insertion needed a boundary correction; current comment/string/code behavior has hosted evidence in `45258023487`. Unsupported/stale snapshots remain conservative literal. |

## Current runtime evidence, narrowly scoped

- `45222480348`: local exact-path Ctrl-click after unsaved edits and tab changes.
- `45227805595`: deterministic LSP cross-file equal-coordinate definition route.
- `45231386808`: old delayed LSP response ignored after edit; fresh click navigates.
- `45236112616`: brackets enabled, quotes disabled, closer skip disabled through live settings reload.
- `45240357001`: Smart Enter disabled with indent retained, then indentation disabled.
- `45243124335`: escaped quote literal insertion and explicit selected-range replacement.
- `45258023487`: PHP comment/string/code partner behavior.
- `45275627813` and `45279064490`: structural matching highlights on/after brackets.

These references do not cover all controls on their screenshots. Add direct source/destination evidence and issue details as each surface is exercised. No surface reaches complete acceptance until its visible-control inventory and required states are checked, and no known broken interaction is ignored or relabelled PASS.

## Settings visible-control inventory, first pass

Source: `lapce-app/src/settings.rs`. All rows below require runtime evidence; source routes are recorded to avoid overlooking controls, not to mark them PASS.

| Control group | Handler/data flow | Outstanding runtime coverage |
| --- | --- | --- |
| Section sidebar | `settings_section` -> measured section scroll target | Pointer and keyboard activation, selected state, all sections, narrow and filtered navigation |
| Search | local editor buffer -> filtered row list | Match/no-match/reset, section title, plugin section, stale scroll offset, search focus and resize |
| Generic text/numeric | 500ms debounce -> `LapceConfig::update_file` | Validation, failed write, dirty draft/reload, workspace override notice, restart persistence |
| Generic boolean | checked signal -> `update_file` | Keyboard/pointer, focus visibility, persistence, disabled/error semantics |
| Generic dropdown | overlay -> `update_file` | Open/close, selection, keyboard arrows/Enter/Escape, edge placement, focus return, resize, failed writes |
| AI advanced/restore | toggle legacy visibility / shared-connection false | Preserve saved routing, accessible focus, feedback and restart |
| AI Local/Remote pending/apply | buffered fields -> apply_shared_ai | No opening-time mutation, layout, key masking, focus, failed save, explicit apply disclosure |
| AI remove key/test | credential store / existing test worker | Permission/state/error/focus, busy disabling; no remote test without authorized scope |
| Local model download/select/cancel | explicit worker / model config / child cancel | Availability, keyboard, disabled/busy, no accidental network download during ordinary UI traversal |
| Theme-color search/input/reset | config colour edits / reset_setting | Long values, color preview, keyboard reset, error feedback, both themes |

Known source-audit gaps to reproduce: generic value writes can fail without inline feedback; generic dropdown has no explicit arrow/escape selection handler in its custom overlay; section selection while search hides that section has no clear action/feedback. These are OPEN under CFG-001/002, not ignored or treated as accepted behavior. Existing AI key storage reports a private plaintext file, not encryption. Secure credential redesign remains an explicit later security ticket and must not be advertised as already encrypted.

## Runtime follow-up, 23:39

`45286820403` verifies UI-001 corrected: the shortcut jumps from after the closer to line 2 column 1, returns to the closer without selecting, and Shift-arrow still selects. `45287670861` verifies no-result guidance and filtered section headers. Its narrow light-theme capture exposes excessively cramped settings content with explorer, terminal and AI panels open. Compact settings navigation now moves above content below a 480px settings viewport, preserving section access without spending half the content width on the sidebar. New runtime evidence is required before acceptance. Full Phase 1 remains OPEN.

`45288278519` has passing Rust tests but failed the real keyboard persistence gate: Tab from the settings search did not focus the filtered boolean target, Space did not change the setting. Screenshots 17a/17b show the value unchanged. Keyboard accessibility is NOT ACCEPTED. Fix focus routing before proceeding to new features; do not weaken the assertion or infer a pass from keyboard_navigable declarations.

`45292659897` actual images 17a/17b verify native Tab reaches the boolean target with visible focus, and Space disables it; the disk persistence assertion passes. The dropdown captures did not reach the dropdown because settings reload rebuilt the focused row; they are NOT evidence of dropdown acceptance. Reacquire search for the independent dropdown test. Compact dark/light images from `45290865787` show top section navigation and wider content without the prior sidebar squeeze. Overall Phase 1 remains open.

## Owner feedback, 24 November 00:13

UI-009, medium/P0: the AI composer's one-line footer reserved 48px plus 8px gap and bottom padding. Remove that minimum, retain natural wrapping for longer keyboard guidance, and use 4px gap/bottom padding. Requires dark wide/narrow runtime screenshots before acceptance.

UI-010, preview-theme confusion: the screenshot supplied to the owner was the explicit light-theme acceptance state, not a change to the shipped default. `defaults/settings.toml` selects Lapce Dark, and config initialization has a dark fallback. Keep testing light, then restore the disposable profile to dark and capture dark final states. Do not overwrite users who explicitly selected another theme.

`45295688323` verifies UI-009 compact disclaimer at dark wide/narrow sizes, and final dark restoration for UI-010. `45295024939` verifies the actual dropdown popup, arrow highlight and Escape cancellation with unchanged selected value. Wrapped focused-footer guidance remains a separate runtime gate, not established by unfocused screenshots.

UI-011, medium/P0: selecting a settings section while search excluded its header had no useful destination. Section activation now clears the search query before using reactive section offsets. Runtime acceptance must show filtered font settings, then Terminal navigation with empty query and actual terminal settings. `45299561539` verifies focused composer footer wrapping at both widths without clipping or sending.

`45302425777` verifies UI-011 search clearing and real Terminal destination after a sidebar click. Dropdown arrow/cancel evidence is separate from persistence; a final keyboard accept/persist assertion and screenshot now exercise choosing vscode. No whole-settings acceptance is inferred.

UI-012, medium/P0: numeric settings previously ignored invalid input and write failures without feedback. Text/numeric rows now show a wrapping error when parsing or saving fails, leave the old persisted value unchanged, and reject non-finite float values. Runtime error display is a new open gate. `45305169812` verifies existing dropdown keyboard acceptance and settings-file persistence.

UI-013, medium/P0: boolean and pointer-dropdown controls changed their visible value before knowing whether the file write succeeded; keyboard dropdown failures were silent. Generic controls now retain their last saved state on failure and show an inline error. Read-only disposable-profile GUI scenarios cover both controls with byte-for-byte settings assertions. Numeric validation now uses Tab from search instead of a coordinate click, and asserts invalid input does not change settings. These captures remain unaccepted until actual pixels and hosted tests are reviewed.

`45309283043` verifies UI-012/013: invalid numeric feedback, unchanged persisted settings, read-only boolean and keyboard-dropdown failures retain their prior state with errors. Actual dark screenshots 21a/22/22a inspected.

UI-014, medium/P0: shared actionable toolbar icons lacked keyboard traversal/focus; popout menu defaults handled pointer activation but not focused keyboard activation. Add traversal/visible focus for actionable icons and title menus/search, route focused Enter/Space to popout menus, and remove duplicate separator in Settings menu. Runtime toolbar keyboard/menu acceptance is required; broad workspace acceptance remains open.

`45311845774` verifies visible keyboard focus on the main menu and Space opening it. The screenshots were initially named workspace-menu, but actual pixels show the main menu; they do not verify the workspace dropdown. Rename captures accurately and continue toolbar traversal/menu checks. Full Rust/release workflow is green.

`45314382956` actual toolbar traversal screenshots map focus to main menu, remote, back, search, workspace dropdown and Run/Debug. The initially named remote-menu capture has no open menu, so it is not accepted. Native menu dismissal resets focus; new captures reacquire targets based on observed traversal and verify actual menus rather than assuming focus retention.

`45316862316` verifies keyboard opening of the actual Open Folder / Open Recent Workspace menu (25a). Remote capture still reaches the main menu, so remote acceptance remains open and its reacquisition sequence is corrected without asserting a false pass. Native popup dismissal/focus return remains a follow-up issue under UX-013.

UI-015, medium/P0: Linux popup Escape left focus on the now-hidden menu, making subsequent Tab restart unexpectedly. Store the opening focus target and restore it on Escape. Runtime captures must show main-menu focus restored, Tab reaching Remote, actual remote menu, then Escape returning focus. Opening a menu is not evidence that its item keyboard navigation or actions work; those remain unverified.

`45320561657` verifies UI-015: Escape returns visible focus to main/remote triggers and the expected Tab target opens the real remote menu.

UI-016, medium/P0: Linux menu rows had pointer handlers but no arrow selection/activation. Add enabled-row keyboard selection, wrapping Up/Down, Home/End, Right/Enter submenus, Left parent, Enter/Space leaf activation and Escape focus return. Skip separators/disabled rows. Runtime captures must inspect selection and submenu open/back, with no remote connection activation. Leaf-action acceptance remains separate.

`45323098016` verifies UI-016 arrow selection, Right opening the Gravity Editor submenu with About selected, and Left returning to parent. Add nested Settings navigation across a separator, End selection, and Enter opening Keyboard Shortcuts to verify a real safe leaf action. Remove remaining Lapce branding from Quit/Hide labels. No remote action is activated.

UI-017, medium/P0: project search showed no guidance for empty/no-match/loading/error states and accepted old replies after later edits or clearing. Add visible wrapping status, regex validation and per-query generation guard; clear old results before new requests. Runtime captures must show genuine no-result and cleared-query states. Search/replace scope and real large-project performance remain open.

`45325538651` verifies nested Settings keyboard navigation, separator skipping, End selection and Enter opening the actual Keyboard Shortcuts view. Search-status first gate failed Rust type inference for the new error branch; add the explicit RPC result type and status grammar tests before runtime acceptance. No search success is claimed from the failed gate.

`45328033432` verifies project-search no-match and cleared-query guidance in actual dark UI. Improve status foreground contrast and capture positive match counts plus regex-error/clear states. Generation-guard delayed-response integration coverage remains open; no empty-state screenshot proves that race is exercised.

`45330444266` verifies positive workspace search: 11 fixture matches in 11 files and readable status. Artifact uploads succeeded. The intended invalid-regex capture still shows a literal bracket result, so regex validation is NOT runtime accepted. Use keyboard toggle/focus captures and retype a different invalid expression to distinguish control targeting from reactive state issues.

UI-018, medium/P0: actual 31 capture shows Tab inserted spaces into the project-search query instead of moving to option buttons. The form input had deferred Tab, but the workbench-level panel binding consumed it as editor indentation. Bypass that binding for native form traversal, including search-panel focus. Regex keyboard focus/error acceptance remains open until rerun. `45333034590` full build passed but its regex workflow is not accepted.

UI-018 follow-up: `45335551758` proves Tab no longer inserts spaces, but actual focus starts from the toolbar because the search input itself was omitted from traversal. Add keyboard_navigable to the input and explicitly reacquire it before option traversal. Regex acceptance remains open; the green build is not a visual pass.

`45338002057` verifies query Tab reaches Regex, Space enables it and actual [ / ( invalid-expression errors display. UI-018 keyboard query traversal is accepted for this path.

UI-019, medium/P0: search file-group and match rows were pointer-only. Add native keyboard reachability with visible focus and preserve existing expand/jump actions. Capture file-group focus/collapse/reopen, result focus and Enter destination. Virtualized offscreen traversal, precise match-column navigation and delayed search races remain open.

UI-017 regression follow-up: route success, error and unexpected-response handling through a single generation-checked boundary. Add deterministic out-of-order reply tests covering stale success, stale error, wrong response kind and generation wrap, plus current success/error distinction. This covers reply acceptance policy, not transport scheduling or a runtime delayed-search race.

UI-020, medium/P0: shared foldable-panel section headers had only pointer activation, with no native keyboard target or focus feedback. Add native traversal and selected-background focus feedback without changing header geometry. Verify actual Open Editors collapse and Space re-expand/collapse; other panel sections inherit the component but remain individually unverified.

`45341285950` builds/tests pass, but actual 32a shows Space also inserted into the query after the row action, rebuilding results and losing focus. Result activation is NOT ACCEPTED. Stop focused row activation propagation after Floem's default click dispatch; preserve Tab traversal. Header captures were obscured by the resulting main menu, so reacquire with Escape before the independent header test.

UI-020 follow-up uses the same post-click activation boundary for panel headers, preventing Space/Enter from reaching an editor still logically focused behind the header. Native Tab remains unconsumed. Runtime capture pending.

`45343379570` verifies UI-019: visible group/match focus, Space collapse/reopen without query edits, and Enter opens the actual matched file. Shared header keyboard collapse is also observed. d375c65 reply-policy tests pass; transport-level races remain open.

UI-021, medium/P0: search matches jumped to first nonblank instead of the matched column, and truncated long-line previews retained offsets from the original line, hiding or misplacing highlights. Preserve original UTF-16 column separately from preview-relative byte offsets. Add Unicode/long-line regression gates and inspect exact destination in runtime capture.

`45346172016` stopped at the new Unicode test: expected column was miscounted as 5; é + surrogate-pair emoji + space is 4 UTF-16 units. Correct the test expectation, retain the passing long-line offsets test, and add runtime unique long-line preview/keyboard-destination captures. This failed run is not acceptance.

`45347459958` passes new Unicode/long-line tests and actual search jumps now show byte column 10 for the short fixture and byte column 331 for the UTF-8 long fixture, matching the true offsets. The long preview still ellipsizes before reaching the match when retaining 100 preceding characters. Reduce leading preview context to 32 characters so the match remains visible at ordinary panel widths; add an unobscured destination capture after dismissing in-file find. In-file find's stale No Results state remains a separate issue.

`45350307713` verifies UI-021: actual long-line preview contains the highlighted match, keyboard activation places the caret at that original text after a Unicode prefix. Both focused regression tests/full suite/release pass.

UI-022, medium/P0: in-file find count remained No Results after a project-search jump to a file containing the query. The count memo read the active editor untracked, keeping dependencies on the previous document. Track editor replacement so count dependencies follow the destination document. Verify existing 32c/34b captures show 1 of 1 rather than No Results.

`45353290145` count still reads No Results in actual pixels, so UI-022 is not accepted. Matching was started only during painting; schedule it explicitly from document/query/buffer/visibility changes instead, and ignore async results after query or buffer revisions change. Retain tracked destination editor count dependencies. Runtime retry required.

`45356437620` verifies UI-022: real destination documents now show 1 of 1 and highlights, including long Unicode text. Full test/release gates pass.

UI-023, medium/P0: cursor/line-ending/language status actions were pointer-only. Make shared actionable status text keyboard reachable with visible focus and activation propagation guards. Runtime check line-ending palette, Escape then Tab language focus and Enter language palette; do not change either setting.

`45361313625` opens the real line-ending palette, but Escape/Tab then indents the source instead of moving to Language. UI-023 keyboard flow is NOT accepted. Register status focus for form traversal and restore the invoking status control after its palette closes; keep both settings unchanged.

`45364855777` still loses status focus to the root Escape key handler, so no keyboard acceptance. Remember the pointer invocation directly and defer restoring focus until after key dispatch, guarding that the palette remains closed. Retry the same workflow.

`45369985403` verifies UI-023: Escape from line-ending palette restores LF control; Tab reaches PHP and Enter opens the real language palette. Source remains visually unedited. Extend cursor-action coverage via Shift-Tab twice and Enter to Go to Line.

UI-024, medium/P0: diagnostic status target omitted from keyboard traversal. Add focus styling and native activation, preventing activation from also editing the code behind it. Its actual keyboard destination remains a runtime gate.

`45373923548` verifies reverse traversal from Language through LF to the cursor status, Enter opens the actual line palette with a source preview. Add diagnostic target capture after reverse traversal across the three center panel toggles, then Enter Problems. Diagnostic acceptance remains pending.

`45377964611` verifies UI-024: actual diagnostic-count focus and Enter opens the real Errors/Warnings panel without visible source changes. All artifact uploads succeeded.

UI-025, medium/P0: branch status was pointer-only and used custom pointer tracking. Reuse status palette focus/activation handling for this compound control, including Escape return, visible focus and activation propagation stop. Runtime must show actual branch focus, SCM references palette, and Escape return without selecting a branch. No source-control change is authorized.

`45381445187` verifies UI-025: keyboard branch focus, real SCM references palette with main/origin-main and Escape returns branch focus. No reference selected. Full test/release/upload gates pass.

UI-026, medium/P0: empty Errors/Warnings sections were blank, with no indication that diagnostics depend on language-service reports. Show severity-specific reported-state guidance only while that section has no diagnostics, tracking diagnostic publication changes. Do not claim a completed clean-code check. Unit wording check and actual hosted empty-state capture required; nonempty/clear transition remains open.

UI-026 acceptance extension: deterministic local stdio LSP fixture now publishes one synthetic error and warning for diagnostics.py, then clears them on text change. Framing/publish/clear smoke test passes locally. Add actual populated and cleared panel captures to verify empty guidance reacts to live RPC notifications. This is UI/RPC fixture coverage, not evidence of a real language provider or a completed code check.

`45386313931` actual 39 shows readable empty guidance, but the in-flow long labels expand the bottom panel and squeeze the explorer/assistant. UI-026 is NOT accepted. Keep content in the section's absolute-size viewport as before, bound label width and explicitly wrap text. Recheck layout plus pending live publication/clear captures.

`45389619554` verifies UI-026: wrapped empty guidance retains normal panel widths; actual local LSP publication shows one error and warning without guidance, then an empty notification restores guidance. All test/release/upload gates pass.

UI-027, medium/P0: Problems file-group, diagnostic and related-location rows were pointer-only. Add native keyboard focus, visible focus, and activation propagation guard; register row focus so Tab does not indent the document. Capture warning file-group collapse/reopen, Tab to warning, and Enter exact warning-line destination. Related-location and offscreen traversal remain unverified.

`45394796677` verifies UI-027 for file-group collapse/reopen, native warning-row focus and Enter destination line 2 column 3. No source edit from activation. Related-row acceptance remains pending.

UI-028, medium/P0: related diagnostic rows reused one constant list key and displayed zero-based positions. Use URI/range/message identity and display one-based positions, preserving zero-based LSP jump coordinates. Add two related locations in the local fixture, native Tab between them and Enter the second target at line 3 column 5. Runtime list identity and destination acceptance required.

`45400010169` verifies UI-028 navigation: two distinct related rows show [1, 1] and [3, 5]; native Tab reaches the second row and Enter opens related_target.py at line 3 column 5. Actual labels are too dim against the panel. UI-029, medium/P0: use normal panel foreground for actionable related text, keeping the decorative link icon dim. Recapture the same actual workflow before contrast acceptance.

`45405535031` GUI 42a/42b verifies normal-foreground related text is readable; the second related row still opens line 3 column 5. Full release completion remains awaited at capture inspection. Add empty Problems narrow dark/light, wide light and restored wide dark captures to check wrapping within the existing panel layout.

`45409417065` full gates pass and actual wide dark/light empty states are readable. Narrow captures expose excessively tall word-by-word wrapping in the two side-by-side severity columns. UI-030, medium/P0: stack Problems sections vertically below 480px panel width and move the divider to the bottom edge. Verify actual narrow dark/light and wide restoration before responsive acceptance. Other panels are unchanged.

## Owner clarification, 24 November 14:45 GMT

UI-002 / UX-004 navigation backlog: Ctrl+click on a Laravel Model, class or route reference must resolve the target and open it in a new tab, rather than replacing the current tab. This is an existing workspace/navigation interaction under Phase 1, not a new autocomplete or AI-agent feature. Verify Laravel-specific reference resolution and tab preservation separately, using a real project/provider before claiming Laravel support. Existing deterministic LSP navigation captures do not close this request. Keep strict phase order.

UI-031, medium/P0: definition navigation passed ignore_unconfirmed=false, allowing an untouched preview source tab to be replaced by its destination. Set ignore_unconfirmed=true for destinations throughout go_to_definition, covering existing local resolvers, LSP definitions and reference destinations. Existing target tabs remain reusable; no duplicate tab is required. Verify source and target coexist visibly in the Ctrl+click local/LSP captures. This tab policy does not establish Laravel resolution correctness.

`45419141988` verifies UI-031: actual local file-reference and deterministic LSP Ctrl+click show both original source and destination tabs, full gates green. Add conventional Laravel-shaped Post::class, ReceiptService::class and route('users.index') local fixtures to exercise existing resolvers through the actual gesture. These are bounded convention checks, not real-project/provider acceptance or proof that arbitrary class references resolve.
