# Toast auto-dismiss for transient INFO messages (item 403)

Toast notifications used to stay on screen until the user dismissed
every one of them, so transient INFO results ("Loaded N artisan
commands...", language-server chatter) accumulated over the editor.
Now INFO toasts dismiss themselves after 8 seconds; warnings and
errors stay until dismissed so important notices are never silently
lost.

Every push gets a unique instance id and the expiry timer removes
only that id, so a timer firing for a toast that was dismissed
early - or drained by the 3-toast cap - can never remove a newer
toast, including an identical re-push. The exact-content dedup keeps
the original instance (and its timer) when a duplicate arrives.

Implementation: `toast_ttl`, `next_toast_id`, `remove_toast` and the
per-instance id in `window_tab.rs`; the timer uses
`floem::action::exec_after` as elsewhere in the app.

Tests: `only_info_toasts_expire`,
`dedup_keeps_the_original_instance_identity`,
`stale_timer_preserves_a_newer_identical_toast`,
`removing_an_absent_id_is_a_noop` (plus the updated dedup/cap tests).
