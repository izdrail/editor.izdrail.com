# Agent chat basics

Open the Agent panel. To configure its connection, click the gear or connection summary, or run **Agent: Open AI Settings** from the command palette. The summary shows configured routing, not a successful connection test. See [local model setup](local-model-setup.md).

## Enter an endpoint key

Use **API key - enter or replace** directly in the Agent panel. The field masks typed and pasted text and never shows a saved key. Backspace removes the last character; Esc clears the unsaved entry. **Save key for this endpoint** stores it locally for the displayed endpoint, without sending a request or switching your model/backend. Local mode does not use an API key.

An empty entry does not remove or overwrite a saved key. Closing the entry or changing endpoint clears the unsaved text. Environment overrides still take priority; the entry shows which key source is active. Use AI settings for endpoint/model setup, connection tests or explicit key removal.

## Start with a draft

An empty conversation offers **Explain this file**, **Review for bugs** and **Suggest tests**. Clicking a starter appends a plain prompt to the composer, keeping any text already there. It does not send anything or change your mode. Edit the draft before sending.

With the composer focused in an empty conversation, **Alt+1** appends Explain, **Alt+2** appends Review and **Alt+3** appends Tests. These are composer-local shortcuts, not global keymap bindings. A desktop shortcut or keyboard layout may intercept them; the buttons remain available.

The card above the composer shows the selected mode's scope and **Attached on Send** context. Check the file or selection shown there before sending. It follows the active editor context; choosing a starter does not pin a particular file.

Press **Enter** or click **Send** when ready. **Shift+Enter** inserts a newline. While a response is running, **Stop** cancels it. Cancellation does not undo changes you already accepted.

## Conversation and review

The heading follows the first user message. Assistant answers sit directly on the panel; your messages use right-aligned bubbles. Mode and model controls sit below the input. While a request is running, the composer send button becomes a red Stop button. It cancels the current request, not an accepted edit. When a live request ends, its measured duration appears as **Worked for**, **Stopped after** or **Failed after**. This is request time, not a claim that edits were applied. A new request/chat clears the old duration; saved-history previews do not invent a duration.

A staged edit proposal shows its real file count and added/removed line counts, with **Review** opening the existing diff review. "Proposed" does not mean applied. Checkpoints show their own state separately. An ordinary Chat answer has no staged diff and does not need edit review. **Prepare review** accepts changes JSON from the composer; plain prose produces an explanation without changing any files.

Changed hunks show added lines in green and removed lines in red. Each file shows both full proposed line totals and the current selected-hunk/line totals. **Accept file** and **Reject file** select or clear that file's hunks without changing buffers; other files keep their selections. **Accept** and **Reject** set each hunk's selection; **Apply accepted hunks** commits that selection to open buffers. **Accept all** applies the whole displayed proposal through the same stale-buffer and checkpoint checks. **Reject all** drops the proposal without changing buffers. These controls never save source files. Per-hunk controls are available in the Agent panel and beside visible changed hunks in the main-editor proposal diff.

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
