# Agent chat basics

Open the Agent panel. To configure its connection, click the gear or connection summary, or run **Agent: Open AI Settings** from the command palette. The summary shows configured routing, not a successful connection test. See [local model setup](local-model-setup.md).

## Enter an endpoint key

Use **API key - enter or replace** directly in the Agent panel. The field masks typed and pasted text and never shows a saved key. Backspace removes the last character; Esc clears the unsaved entry. **Save key for this endpoint** stores it locally for the displayed endpoint, without sending a request or switching your model/backend. Local mode does not use an API key.

An empty entry does not remove or overwrite a saved key. Closing the entry or changing endpoint clears the unsaved text. Environment overrides still take priority; the entry shows which key source is active. Use AI settings for endpoint/model setup, connection tests or explicit key removal.

## Start with a draft

An empty conversation shows the workspace name and a short prompt above the bottom composer. Long workspace names wrap within the panel instead of widening or clipping the welcome area.

With the composer focused in an empty conversation, **Alt+1** appends Explain, **Alt+2** appends Review and **Alt+3** appends Tests. These are composer-local shortcuts, not global keymap bindings. A desktop shortcut or keyboard layout may intercept them. These shortcuts append a draft without sending it or changing the mode.

Open **Conversation tools and settings** to see the card above the composer with the selected mode's scope and **Attached on Send** context. Check the file or selection shown there before sending. It follows the active editor context; choosing a starter does not pin a particular file.

Press **Enter** or click **Send** when ready. **Shift+Enter** inserts a newline. Ctrl, Alt or Super/Command plus Enter is not a Send shortcut. While a response is running, **Stop** cancels it. Cancellation does not undo changes you already accepted.

## Conversation and review

The heading follows the first user message. Assistant answers sit directly on the panel; your messages use right-aligned bubbles. Mode and model controls sit below the input. While a request is running, the composer send button becomes a red Stop button. It cancels the current request, not an accepted edit. When a live request ends, its measured duration appears as **Worked for**, **Stopped after** or **Failed after**. This is request time, not a claim that edits were applied. A new request/chat clears the old duration; saved-history previews do not invent a duration.

A staged edit proposal shows its real file count and added/removed line counts, with **Review** opening the existing diff review. "Proposed" does not mean applied. Checkpoints show their own state separately. An ordinary Chat answer has no staged diff and does not need edit review. **Prepare review** accepts changes JSON from the composer; plain prose produces an explanation without changing any files.

Changed hunks show added lines in green and removed lines in red. Each file shows both full proposed line totals and the current selected-hunk/line totals. **Accept file** and **Reject file** select or clear that file's hunks without changing buffers; other files keep their selections. **Accept** and **Reject** set each hunk's selection; **Apply accepted hunks** commits that selection to open buffers. **Apply all hunks** applies the whole displayed proposal through the same stale-buffer and checkpoint checks. **Reject all** drops the proposal without changing buffers. These controls never save source files. Per-hunk controls are available in the Agent panel and beside visible changed hunks in the main-editor proposal diff.

**Open read-only editor diff** opens full before/proposed snapshots in a main-editor diff tab. They are not the live source buffer and cannot be edited or saved as files. Accept/reject stays in Agent review, which still checks the current source before applying. Snapshot text is not restored after restart; a reopened preview tells you to return to a current proposal.

## Pick the right mode

- **Chat** uses conversation and attached context, without the project tool loop.
- **Ask** uses project read/search tools, without proposed file edits.
- **Plan** uses project read/search tools and can save a plan artifact, without source edits.
- **Act** can propose edits for review. Commands and MCP tools require approval per call.

A mode label is not a permission grant. Keep the existing project-read and change-review controls in mind. Review proposals before applying them.

External ACP agents are separate. Their runtime owns their tools and permissions; configuring Gravity's shared connection does not configure an external agent's model or sign-in. Native review remains separate.

## Browse without resuming

Use **Saved chats - browse this workspace** to preview saved native conversations. Opening a preview keeps your current draft, conversation, model, mode and approvals unchanged. It does not resume tools or a model request. See [saved chat previews](saved-chats.md) for limits and history settings.

The **External agents** list has separate ACP session controls. Do not confuse a native saved preview with resuming an external agent.

The read-only main-editor proposal diff now has **Accept file**, **Reject file** and **Apply selected** controls. File selection is shared with the Agent panel, including per-hunk choices. Apply selected applies all currently selected hunks across the proposal to open buffers only. The toolbar disables actions when its snapshots no longer match the pending proposal. After apply or discard, the snapshots remain read-only, with no active review controls. Visible changed hunks also have Accept and Reject buttons in the proposed editor, sharing the same selection. They follow diff scrolling and disappear when the proposal no longer matches the snapshots.

New conversations keep the project title, a short prompt and the bottom composer. The header's Conversation tools and settings button opens connection/key controls, saved chats, external agents, plans, manual review and context details. Prior checkpoints do not open themselves into a new chat. Pending proposals, errors and running work remain visible. Ask/Plan still show the complete project-read approval before use; hiding routine Chat chrome never grants permission.

The conversation header stays above the scrolling body, so New conversation, tools and Stop remain reachable during long reviews and conversations. The composer stays pinned below it.

New conversation also closes mode/model/mention popups and focuses the empty composer, so you can start typing without an extra click. It does not apply or discard pending edits.

Clearing the composer resets its caret to the start of the empty draft, including after undo, so the next keystroke stays inside the document.

An empty or whitespace-only draft keeps Send inactive. Clicking Send or pressing Enter on it does nothing and does not add an error notice. Stop remains available while work is running. Shift+Enter still inserts a newline.

Escape, Tab or Shift+Tab closes the composer mode or model menu and returns keyboard focus to the draft. Clicking outside still leaves focus on the place you clicked.

Choosing a mode, external-agent preset or model also returns focus to the existing draft. Picking a menu item does not send the draft or grant project access.

Main-editor proposal diffs reserve a separate rail for visible hunk decisions, so long proposed lines do not run behind Accept/Reject. Proposal snapshots scroll horizontally rather than soft wrap. The rail is released when no pending proposal matches those snapshots.

The review section shows preparation instructions only when no proposal or checkpoint is present. Active reviews keep their hunk controls and buffer-only warning, without repeating setup instructions above them.

Dismissing a checkpoint replaces old restore instructions with a confirmation that buffers are unchanged and one-click restore is no longer available. If journal removal fails, the checkpoint and its recovery guidance remain.

"Apply all hunks" applies the whole proposal to open buffers immediately. Per-file and per-hunk Accept/Reject only change the selection; "Apply accepted hunks" applies that selection. Neither action saves source files.

The welcome prompt appears only in an idle, empty conversation. Pending review, active work and errors hide it so it does not compete with the work that needs attention.

New conversation clears old preparation errors and notices when no proposal or checkpoint remains. Pending proposals and checkpoint recovery guidance stay intact.

The welcome title and hints stay centered as they wrap, with separate spacing between each line of guidance. This layout does not change the draft or send it.

The composer mode and model chips can receive keyboard focus. From the draft, Tab focuses the mode chip and Shift+Tab focuses the model chip. Tab cycles through mode, model and back to the draft; Shift+Tab follows the reverse order. Press Enter or Space to open a chip menu. A focus border identifies the current chip. Escape returns to the draft from a focused chip or an open menu without sending it.

File suggestions accept only plain Enter or Tab. Shift+Tab closes suggestions and moves to the model chip without completing the mention. Shift+Enter keeps its newline behavior while suggestions are open. Other modified Enter/Tab shortcuts do not accept a suggestion.

Conversation text wraps to the agent panel width, including formatted assistant answers and your message bubbles. Narrowing the panel keeps the full text readable instead of clipping long lines.

While file suggestions are visible, plain Up/Down changes the suggestion and plain Escape dismisses it. Modified Up/Down stays with the editor, so Shift+Up/Down can select draft text instead of changing a suggestion.

Use **Copy** beside an assistant answer to copy its original text, including Markdown and code fences, to the clipboard. **Copied** confirms success; **Copy failed** means the clipboard was unavailable. Copy does not send anything, change the draft, or apply edits. The empty conversation has no copy controls.

Your message bubbles show the exact prompt as plain text, including Markdown markers, links and code fences. Assistant answers still render Markdown. This display choice does not change what is sent or saved.

With the mode menu open, plain Up/Down highlights Chat, Ask, Plan, Act, ACP or Resume. Home/End jumps to the first/last mode. Enter picks the highlighted mode and returns to the draft. Navigation alone does not change the mode or start a request; Escape or Tab closes without picking. Agent-specific rows remain available by mouse.

The model menu also supports plain Up/Down and Home/End, skipping unavailable or informational rows. Enter selects only an explicitly highlighted, available model and returns to the draft. Escape/Tab cancels; browsing models does not send the draft.

Space, like Enter, picks an explicitly highlighted mode or model. With no keyboard highlight it does nothing, so the same key that opens the chip does not silently accept a choice. Modified Space never selects a menu row.

Long model labels and menu notices wrap within the menu card instead of running off its edge. The card stays within narrow window bounds, with room for its scrollbar.

With a mode or model menu open, type one unmodified letter or digit to highlight the next matching enabled row. Repeat it to cycle matches, then press Enter or Space to choose. Typing alone never changes the mode or model or sends the draft.

The composer grows for multiline drafts, then scrolls at a fixed height cap so the conversation stays visible. Removing lines shrinks it back. Shift+Enter still adds a line; plain Enter still sends.

The composer grows for soft-wrapped lines too, using the editor's actual text layout and line height. Narrowing the panel grows the draft until its height cap; widening it or removing text shrinks it again.

Accepting a file mention with Enter, Tab or a click moves the caret after the completed path and its trailing space. Clicking a suggestion returns focus to the draft so typing can continue. A stale suggestion does not replace a tail token that has already changed.

File-mention suggestions are active only with one caret at the end of the draft. Moving into earlier text or selecting a range dismisses them, so editing existing text cannot accept a later tail token. Return to the end to continue completing that token.

The file-suggestion list wraps long paths within the composer and scrolls at a fixed height cap. Up/Down updates the highlighted row and scrolls it into view, keeping the selected suggestion visible in a narrow panel.

Suggestion highlighting and automatic scrolling follow the current selected file when typing filters or reorders the results, including when a row was already visible under a broader query.

File suggestions only offer paths that the current @path syntax can attach unchanged on Send. Paths containing spaces, non-ASCII characters, a stripped punctuation suffix or more than 255 bytes, and root-level extensionless names are omitted rather than inserting a token that would silently target another path or attach nothing. Extensionless files inside a directory remain supported.

Keyboard focus cycles from draft to mode, model, Send/Stop and back to draft, with Shift+Tab reversing the order. The action button has a visible focus ring; plain Enter or Space uses the same action as clicking it, while Escape returns to the draft. An empty idle draft remains inactive.

When the Send/Stop button has keyboard focus, the existing footer explains its current action and the Escape route. It tells you to write a message for an empty idle draft, shows Send for a populated draft, and Stop during a run. Leaving the action restores the review-changes reminder without adding another row of controls.

While file suggestions are open, the existing footer shows the selected position among the offered rows and the Up/Down, Enter/Tab and Escape keys. It counts the bounded suggestion list, not every matching workspace file, and returns to its normal hint when the list closes.

A loaded local file query with no offered matches shows a temporary no-matches hint in the existing footer for the current draft query. Changing the query can restore suggestions; adding a space ends the query and restores the normal footer. An index that has not loaded and a remote workspace are not reported as empty results.

The draft focus ring follows the inner text editor, including mouse entry and Escape back from composer controls. Typing in the draft also moves app focus to Chat, so another panel does not remain the active keyboard target. The ring clears when draft focus leaves.

Dismissing a matching suggestion list with Escape is not reported as no matches. The no-match footer uses the actual eligible candidate result and only appears while the draft has focus; switching to another panel restores the regular reminder.

With file suggestions open, plain Home and End jump to the first and last offered file and keep that row visible. Modified Home/End keep their normal editor behavior; Enter/Tab still inserts only the selected row. The temporary footer includes these keys.

No-match feedback checks for the first eligible result without building or sorting a second candidate list. It uses the same path eligibility and query matcher as the ranked suggestions, including after Escape dismisses them.

The file suggestion picker retains only the best offered rows while scanning the workspace. Ranking remains score, shortest path, then path order; the list no longer allocates and sorts every matching workspace path just to offer eight results.

Switching keyboard ownership from Chat to the terminal or source editor dismisses file suggestions and restores the normal footer. A late workspace-index reply cannot reopen the list in another panel. Returning to the draft preserves its text; editing the query opens suggestions again.

Moving the pointer over an offered file highlights that row and updates the position hint without inserting it or stealing draft focus. Enter/Tab can then accept the highlighted row; click still inserts directly. Hover resolves the current path after filtering rather than keeping an old row index.

Opening the mode or model picker dismisses file suggestions first, so two composer lists do not compete for the same space or keyboard hints. Closing the picker preserves the draft; editing its query opens file suggestions again.

The composer footer reserves two lines so opening a picker does not move its trigger under the pointer when temporary file hints clear.

Moving the pointer over a model row updates the keyboard highlight without selecting it. Enter/Space then picks the highlighted enabled model. Disabled, loading or stale rows clear the highlight so Enter cannot choose a previously hovered model.

Mode and model lists prefer opening above their composer buttons, leaving the footer and status bar clear. A button near the top of a short window can open its list below instead; either placement stays within the window edges.

Hovering a built-in mode (Chat, Ask, Plan, Act, ACP or Resume) also updates its keyboard highlight, so Enter/Space picks the row under the pointer. Moving onto a mouse-only agent-command row clears that highlight rather than letting Enter pick the previous mode. Hover alone never changes mode or starts an agent.

In a short window, mode/model lists reduce their scroll area to the available window height rather than keeping a fixed-height card that extends offscreen. The height updates when the window is resized.

Keyboard highlighting in the mode list scrolls the highlighted built-in choice into view, including in a short window. Arrow keys, Home/End and letter jumps do not leave their target hidden above or below the scroll area.

Mode-key navigation validates its current row before acting. An obsolete highlight cannot pick a missing row; arrows and letter jumps restart within the offered choices.

Mode and model letter jumps also accept Shift for uppercase letters. Control, Alt and Meta shortcuts remain untouched; Shift does not activate a highlighted row.

Agent command rows participate in mode-menu arrow, Home/End and letter navigation. Enter or Space selects a command and returns to the draft, without launching the agent or sending anything.

Page Up and Page Down move through five enabled mode/model choices at a time, skipping unavailable models and stopping at list boundaries. These keys only move the highlight; Enter or Space is still required to select.

The @file suggestion list also supports Page Up and Page Down in five-row steps. Paging moves only the highlight, never inserts a mention or sends the draft.

Opening the model list starts the keyboard highlight at the currently configured model when it is available and enabled. Enter confirms that model; arrows move from it. Loading, unavailable and disabled models never become an implicit selection.

The mode menu also starts at the active mode, Resume choice or configured built-in agent command. Opening a menu never changes the mode or starts a run.

Agent command letter jumps match the agent name as well as the visible Agent prefix. O reaches opencode; repeated C cycles Chat, Claude Code and Codex. No command is chosen until Enter or Space.

Model picker refreshes preserve the highlighted model by ID, even if rows reorder or capability labels arrive. If that model disappears or becomes unavailable, selection clears instead of pointing Enter at another model.

Model rows rebuild their event handlers and highlight/scroll state when their position or availability changes, even when the visible name stays the same.

When a composer mode/model chip has focus, Left and Right move focus along the controls without opening a menu, changing a choice or sending. Left from the mode chip returns to the draft; Right from the model chip focuses Send. Tab and Shift+Tab remain available.

Left from the focused Send/Stop button returns to the model chip; Right returns to the draft. These arrows only move focus, including while a run is active. Enter/Space remain the explicit Send/Stop actions.

The @file suggestion footer lists PageUp/PageDown alongside single-row and end jumps, including the five-row page size. These keys only move the highlight; Enter or Tab inserts the file path.

With a mode/model chip focused, Up or Down opens its choice list at the current choice. It does not pick a different mode/model or send the draft. Enter/Space still opens the list; modified arrows keep their normal shortcuts.

Left/Right also leaves an open mode/model list along the composer controls: mode Left returns to the draft, mode Right focuses model, model Left focuses mode, and model Right focuses Send. The list closes without accepting its highlighted row or activating Send.

Focusing a mode/model chip replaces the general composer note with its keyboard help. Moving focus away clears that hint, so the quiet new-conversation state keeps its minimal footer.

The focused Send/Stop footer also names its horizontal exits, including when the empty draft cannot be sent: Left returns to model and Right/Escape returns to the draft.

The composer outline stays focused while the draft, mode, model, or Send/Stop control has keyboard focus. Each control keeps its own focus marker so the current target remains clear. Moving focus outside the composer clears the outline.

While a mode or model list is open, the composer keeps its focus outline and the footer explains highlighting, choosing, and returning to the draft without choosing. Closing the list restores the usual focus-specific help.

Composer guidance uses normal text contrast while you are typing, using a composer control, or choosing from a mode/model list. The idle disclaimer stays dim when focus moves elsewhere.

Open mode/model lists advertise PageUp/PageDown jumps of five enabled choices and Home/End jumps to the first/last enabled choice. These keys only change the highlight; Enter/Space chooses.

With the draft focused, the footer explains Enter to send, Shift+Enter for a new line, Tab to mode and Shift+Tab to Send/Stop. Mention feedback and open-list help take priority.

Draft guidance follows the current state: an empty draft asks for a message; a reply in progress offers continued drafting and Shift+Tab to Stop rather than promising another send.

File suggestions belong to the focused draft. Moving to a composer control clears them, and a late file-index response cannot reopen them while focus is elsewhere. Returning to the unchanged draft can show matching files again.

While the workspace file index is loading, the focused @ query says that no file has been attached yet. Without an open local folder it asks for one instead. After the index arrives, a no-match message distinguishes a query from an inserted file reference.

A failed file-index request says that suggestions could not load, rather than claiming loading is still in progress or that no files match. Remove the active @ query and type it again to retry. Typing more characters in the same failed query does not keep retrying.

With a failed @ query focused, F5 retries the file index without changing the draft. Modified F5 and ordinary composer keys keep their normal behavior. A failure that arrives after the query was removed also resets for the next query.
