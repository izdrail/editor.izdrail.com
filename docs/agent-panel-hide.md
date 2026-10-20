# Hide the Agent panel (item 307)

The Agent (right) panel can be collapsed from inside itself: click the "Agent" title or the "Hide agent panel" icon at the right end of its header. Both call the same `toggle_panel_visual(PanelKind::Chat)` that the panel's tab-strip icon uses, so hiding from either place is equivalent. To bring it back, click the Agent icon in the right tab strip, the right-panel toggle in the status bar, or use the existing toggle command. The shown/hidden state is part of the saved panel layout, so it survives a restart.

Host capture: click the title (panel collapses, editor widens), click the right tab-strip icon (panel returns with the same chat), restart and confirm the last state is kept.
