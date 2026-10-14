# Reviewed buffer edits and checkpoints (4b candidate)

Act is disabled. This milestone is a review/apply surface, not autonomous edit
execution. Ask/Plan stays read-only. Hosted providers remain off.

## Review

Open an existing local project and the files you want to edit. On Linux, turn
off editor autosave first. In the chat composer paste explicit JSON, then click
"Prepare review from composer JSON":

```json
{"changes":[{"path":"app/User.php","replacement":"full replacement text\n"}]}
```

The input is an explicit full-file replacement. This does not consume model
output automatically. Prepare does not send a model request or change buffers.
The review captures current buffer revisions/text and identity, displays a
changed-region diff plus full before/after text, and gives Apply and Discard.
Apply checks the exact displayed proposal digest and every base again.
Paths are relative to the project; only existing, loaded writable owned file
buffers are accepted. Secure Linux reads deny traversals, symlinks, hardlinks,
mount escapes, ignored files, dot/secret/vendor files, binary and oversized files.
The proposal limit is 32 files, 256 KiB per side per file, 2 MiB total.
Mixed line-ending bases are rejected because restore must preserve exact text.
Replacements are normalized to the buffer's line endings before review.

## Apply and restore

Apply stages detached copies of every buffer before publishing any buffer.
Detached clones do not change the live background-work revision clock. It
writes and syncs a private Prepared journal before mutation. All staged buffers
publish in one synchronous UI callback, then normal editor delta notifications
run. The resulting real revisions/text are recorded in an Applied journal.
There is no source-file save, create, terminal command or Git effect.
Editor autosave must remain disabled for prepare/apply/restore. Normal editor
behavior and settings are not changed by this feature.

Undo is one entry per document. Ctrl+Z affects the current document, not all
files. True cross-document undo requires separate editor-core transaction work.
"Restore this checkpoint" validates every owned buffer, then replaces all owned
buffers with their base text, preserving unrelated buffers and later user work.
Any later revision/text change or closed/reopened owned document blocks a live
restore. Restore also creates one undo entry per document and never saves.

Checkpoints retain base and applied source text in a workspace-scoped local
`chat-sessions/<workspace-hash>/checkpoints/<proposal-digest>.json` directory.
Linux storage uses pinned directory handles, no symlink following, directory
mode 0700 and file mode 0600, exclusive directory locking, synced temporary
files, atomic rename and directory sync. Corrupt/unsafe records fail closed.
History is capped at 32 records without automatic deletion. Archive/remove
reviewed records manually before creating more; old records contain private code.

Restart loads the latest checkpoint for review only. It never edits files or
buffers automatically. An explicit restore after restart requires all target
files open and their text exactly equal to the saved applied text; revision
numbers from a previous process are not reused. If unsaved changes vanished on
restart, restore will conflict rather than replay the proposal. A Prepared
record remains evidence for manual recovery after interrupted application; it
is not proof that application completed. Only the latest record is shown in
this initial UI; older records are retained, not automatically selected/replayed.

## Acceptance still required

Compilation and pure-contract tests do not establish GUI acceptance. Verify
actual screenshots in a compatible host runtime before calling the UI ready:

- Two-file review shows correct paths, changed lines and readable full sides.
- Prepare and Discard leave both buffers and source files unchanged.
- Apply changes both buffers, source files remain unchanged, one Ctrl+Z in each
  document independently restores its base.
- A later user edit on either owned document prevents checkpoint restore on all.
- Untouched checkpoint Restore reverts both owned buffers, not an unrelated file.
- Autosave-on, stale base, missing/read-only/closed-reopened file, symlink,
  duplicate buffer/path, ignored path and corrupt journal all fail closed.
- Restart shows review only, emits no apply or network request; missing applied
  text conflicts. Check private storage permissions and retained record count.
- Long code, narrow panel, Unicode and final-newline differences remain inspectable.

No model quality, real-model edit generation, autonomous Act or full Antigravity
parity is claimed by this milestone.
