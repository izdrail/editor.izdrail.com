# Agents list empty state (item 314)

Opening "Agents (0)" showed nothing, so the click looked broken. The open list now shows "No saved agent sessions yet. Start a chat and it will appear here." (`agents_empty_hint`, unit-tested). The header row also gets a hover background and 4px radius so it reads as clickable.

Capture needed: a workspace with no saved sessions, header clicked open; and one with sessions to confirm the hint is hidden.
