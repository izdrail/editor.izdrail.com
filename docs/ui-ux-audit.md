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
