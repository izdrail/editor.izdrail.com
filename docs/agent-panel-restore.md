# Restoring the Agent panel (item 308)

Host findings after item 307, and what changed:

1. Nothing visible to click after collapsing. A thin 28px edge strip with a "Show agent panel" icon now sits at the right edge while the right container is hidden (`agent_restore_handle` in app.rs). One click shows the Chat panel.
2. Right container drawn with tabs but no content. The cause is not proven from code alone. Two guards were added: the tab view clamps an out-of-range active index to the last panel, and restored styles are clamped (`clamp_styles`). If it still reproduces, capture `workspace` info JSON (panel styles) and the order of events.
3. A fresh "Toggle Right Panel" opened the first tab (blank). A new layout now starts with the right container's active tab on Chat.
4. Restart restored a blank right panel. The restore path built `PanelData` directly, so it skipped the core-panel fix-ups that a fresh layout gets. It now runs `ensure_core_panels` (adds Chat and Plugin if the saved order lacks them) and `clamp_styles` first. The SIGTERM-vs-graceful difference is not explained by this change and needs the saved JSON from both exits.
