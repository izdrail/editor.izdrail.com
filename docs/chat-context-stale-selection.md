# Crash: chat context sliced a stale selection (item 333)

Symptom: accepting a completion with Enter (and Ctrl+A then Backspace with the popup open) crashed the editor in lapce-xi-rope `ChunkIter::next` (rope.rs:570/571, unwrap on None or subtract overflow), called from `slice_to_cow` in the chat panel's `context()` (chat_view.rs, collecting selection regions).

Cause: `context()` runs reactively (the composer's context indicator reads it). It reads the buffer and the cursor, which are separate signals. After an edit that shrinks the text, the buffer signal changes first and the closure re-runs while the cursor still holds offsets from the old text, so `slice_to_cow(min..max)` went past the end of the rope.

Fix: `selection_ranges` orders each region and clamps it to the buffer length, dropping ranges that become empty; `context()` slices only those. The stale state is transient: the cursor update re-runs the closure with correct offsets. Tests: `selection_range_tests` (clamp, drop, reversed, empty buffer).

Not covered: a stale offset in the middle of a multi-byte character after the clamp. That could not be checked here (the rope crate is not in the repo). The other slice sites that read selection regions (editor.rs copy and cut paths) run inside edit commands, not reactive closures, so they see a consistent cursor.
