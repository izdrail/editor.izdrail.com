# Agent chat basics

Open the Agent panel. To configure its connection, click the gear or connection summary, or run **Agent: Open AI Settings** from the command palette. The summary shows configured routing, not a successful connection test. See [local model setup](local-model-setup.md).

## Start with a draft

An empty conversation offers **Explain this file**, **Review for bugs** and **Suggest tests**. Clicking a starter appends a plain prompt to the composer, keeping any text already there. It does not send anything or change your mode. Edit the draft before sending.

The card above the composer shows the selected mode's scope and **Attached on Send** context. Check the file or selection shown there before sending. It follows the active editor context; choosing a starter does not pin a particular file.

Press **Enter** or click **Send** when ready. **Shift+Enter** inserts a newline. While a response is running, **Stop** cancels it. Cancellation does not undo changes you already accepted.

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
