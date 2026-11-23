# UI/UX audit and runtime verification register

Baseline: 23 November 2026, main `0127c7c`. This is an OPEN audit, not a declaration that every control has been tested. It accompanies [the IDE roadmap](professional-ide-roadmap.md) and the existing editing audit. All current captures are hosted CI previews, not deployed/live-product evidence.

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
