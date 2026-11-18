# Zed-inspired Agent UI gap plan

2026-11-18. Behavioral comparison, not a claim of full Zed parity. This editor keeps its own code and its local/Ollama/LM Studio direction. No hosted-provider signup, API call or new permission is introduced by this plan.

| Priority | Zed behavior | Current Gravity surface | Work |
| --- | --- | --- | --- |
| 1 | AI settings from the Agent menu and a direct palette command | Unified AI settings exist, but the panel link appears only for connection errors | Persistent settings action, route/model summary, direct AI command |
| 2 | Ollama and LM Studio setup with model discovery | Shared endpoint form and on-demand model listing exist | Clear local-provider presets and model/endpoint validation, no automatic endpoint change or startup probe |
| 3 | Project-grouped threads and thread history | ACP sessions, run registry and native history persistence exist | Discoverable native thread browser with bounded restore and clear active-run rules |
| 4 | Visible context, profiles and tool support | File mentions and Chat/Ask/Plan/Act/ACP selectors exist | Context chips and explicit read-only versus owner-reviewed change labels, without expanding permissions |
| 5 | Hunk review, thread compaction and context usage | Reviewed full-file proposals and bounded loops exist | Separate correctness work, not a cosmetic parity claim |

## First slice: settings access

The Agent header settings icon and connection summary open the existing AI section directly. The command palette includes **Agent: Open AI Settings**. The summary describes configured routing, not a successful connection test. External-agent configuration remains distinct from the editor's own model connection. Opening settings must never send a prompt, start discovery, retry a failed request or change the endpoint/key.

## Evidence and limits

Compared official current documentation with `panel/chat_view.rs`, `settings.rs`, `chat.rs`, `chat_credentials.rs`, `command.rs` and `window_tab.rs` at the b7da32ab baseline. Zed itself was not run in the GUI. The listed Zed behavior is documentation-grounded; Gravity UI changes require their own tests and actual GUI check.

- [Agent Panel](https://zed.dev/docs/ai/agent-panel): model selector, threads, context, review.
- [Agent Settings](https://zed.dev/docs/ai/agent-settings): direct AI settings entry points.
- [Local models](https://zed.dev/docs/ai/use-a-local-model): Ollama/LM Studio configuration and discovery.
- [Agent Profiles](https://zed.dev/docs/ai/agent-profiles): tool availability differs from permission.
- [Tool Permissions](https://zed.dev/docs/ai/tool-permissions): explicit permission boundaries.
- [External Agents](https://zed.dev/docs/ai/external-agents): ACP runtime/auth/model ownership.
- [LLM Providers](https://zed.dev/docs/ai/llm-providers): model-access paths differ from external agents.
