# Local-only AI defaults (item 280)

Fresh profiles default Chat and ghost text to local CPU inference. Hosted
providers remain disabled. Remote next-edit prediction defaults off. Both Rust
serde defaults and bundled defaults explicitly select local. No automatic
fallback to remote is introduced. Model downloads and connection tests remain
explicit user actions and can use the network; local inference may require a
separately downloaded model before it works.

Existing explicit backend settings are preserved, not silently migrated. Remote
endpoint presets remain selectable but dormant with local backend. Choosing a
remote backend/provider is opt-in. This patch changes defaults, not a global
network firewall (updates, extension downloads and other network features are
not covered by AI backend selection).

Host: fresh/disposable profile must show local for both backends, next-edit off,
hosted providers disabled; sending without local model must not contact remote.
Existing profile explicitly choosing ollama should remain that choice. Capture
normal/narrow AI sections and confirm descriptions match defaults.

## Separate features and private key slot (item 283)

AI Chat is the conversation panel; AI Autocomplete is inline code suggestions.
They are not duplicate backends: independent models and context are used. Keep
both sections, explain them, and group remote connection in its own bordered
card showing the current Chat backend. Both backend selectors offer local first;
autocomplete previously had an unrestricted text input instead of a selector.

The remote gateway credential slot already exists: LAPCE_LLM_API_KEY in the
user's process environment, or the endpoint-bound app-local private credential
file saved by the masked UI. Environment takes precedence. Host injects locally;
never commit an actual value, add it to defaults, or put a secret in a generic
Debug/Serialize settings field. Saving the key while local is selected does not
switch backend or send code. Local CPU inference does not use a gateway key.

Host: normal/narrow cards and wrapping, local-first selectors in both sections,
no key in exported settings/source/defaults, local send remains offline after
saving a remote key. Check saved remote choices remain selected and selecting
local persists. This is grouping phase one, not a full Settings redesign.
