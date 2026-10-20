# Editor UI regression gate

UI correctness is required alongside compile/tests. A patch is not done merely
because it builds. Use the running editor, not a mockup or a source screenshot.
Record build commit, screen size, scale, test project, actions and PNG evidence.
Never include real API keys, private source or personal terminal history in QA
captures. Use fixture projects and fake credentials; verify secrets stay masked.

## Every UI patch

- Open the affected screen at normal and narrow window widths, including with
  explorer/Agent/terminal panels open. Test normal and increased UI font size.
- Inspect readable text, contrast, wrap, buttons, focus indication, clipping,
  overlap and hit targets. No invisible controls under scrollbars or panels.
- Tab/Shift-Tab, click focus, typing/paste, Escape and Enter where applicable.
- Scroll each region to both ends. Wheel must not bleed to an unrelated pane;
  reaching a boundary must not change which section is visible or jump layout.
- Resize while open and after scrolling. Reopen the view and recheck state.
- Capture real PNGs for normal plus the most demanding narrow/scroll/focus case;
  inspect actual pixels. Fix and recapture if any defect remains.
- Report functional gate and visual gate separately. Missing screenshots mean
  visual verification is pending, never an implied pass.

## Full sweep

| Surface | Required checks |
| --- | --- |
| Settings, every sidebar section | Correct heading, first/last fields, search results, dropdown placement, long descriptions and nested scroll containment. |
| AI Chat / Autocomplete | Full model names, FIM/Chat distinction, intro/licence text, wrapping action buttons, top/bottom model rows; connection endpoint/model/masked key, save/remove/test statuses, focus and paste. |
| Agent panel | Long messages/errors, collapsed/expanded progress and review cards, composer visibility, stop/send state, context labels; 401 points to credential UI. |
| Completion popup | Empty/small/large lists, keyboard selection, description panel, screen-edge placement and scroll; no overlap with typed text. |
| File explorer | Deep nesting, long names, selection, context menu and bottom entries with terminal visible. |
| Terminal | Resize, long output, tab labels, scrollback and input focus; no draw outside pane. |
| Status bar / notifications | Narrow width, long statuses, multiple notices, close controls, source attribution and accurate counts. |

## Current known verification gaps

The requested all-surface sweep needs host-run screenshots. This checklist is
not evidence those checks passed. Indexing notifications from a PHP language
server must retain their provenance rather than being presented as the editor's
own index counts. First-open symbol extraction is separate from LSP indexing.
