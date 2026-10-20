# Overlay shadows (item 323)

Hover, rename and inline AI edit overlays had a border but no shadow, unlike completion and code actions (items 311, 318), so they blended into the editor text. They now use the same 6px blur and `LAPCE_DROPDOWN_SHADOW`. The inner rename and AI-edit input borders are unchanged. Capture: hover a symbol, F2 rename and the inline AI edit in dark and light themes.
