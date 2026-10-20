# Layout autosave (item 310)

The workspace layout (panel order, active tab, shown/hidden) used to be written only on a graceful close. SIGTERM, SIGINT and crashes skipped it, so the panel state reverted. A background thread now sends a tick every 30 seconds; the UI thread handles it by running `LapceDb::save_app`, the same save as on window close. The save is queued to the existing db writer thread. No signal handlers were added.

Limit: a change made in the last 30 seconds before a kill is not saved.
