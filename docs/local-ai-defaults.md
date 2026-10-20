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
