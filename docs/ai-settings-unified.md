# One AI settings page (item 287)

AI Chat and AI Autocomplete now share one AI sidebar target and scroll section.
Chat and ghost-text feature/model rows retain their descriptive prefixes. Old
navigation requests for either section resolve to AI. Underlying ai-chat and
ai-autocomplete config keys are unchanged, as are saved values and runtime
routing. Opening this page does not write settings.

This is the page consolidation step, not a shared-transport migration. A routing
notice shows each backend. Legacy ghost backend/endpoint controls are behind
Show advanced legacy ghost connection. They remain available for existing
key-less Ollama profiles. Chat's connection/key does not yet authenticate ghost
requests; a working shared gateway transport and explicit Apply are follow-ups.

Host: compile/test/clippy including legacy_ai_navigation_targets_share_one_section.
Capture normal/narrow sidebar (one AI entry), top card wrapping, advanced closed
and open, and scroll down through both feature/model rows with no overlaps.
Test old Chat and Autocomplete settings links land on AI, search still finds
both legacy prefixes, and editing each row writes its original table. With a
mixed saved profile, open/close and restart must preserve both backend/endpoint
choices byte-for-byte. No send, automatic routing migration or real key is needed.
