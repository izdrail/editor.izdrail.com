# Read/search permission foundation (milestone 3)

agent_read is a Linux-only read/search executor available to future agent code.
It is not connected to the chat UI yet and makes no automatic model or tool calls.
Owner-selected project roots require explicit workspace trust. It cannot write,
run commands or access the network. Workspace trust UI and agent wiring arrive
with the loop; a model must never set the trusted argument itself.

The root is opened as a pinned directory handle. Every file/directory access uses
Linux openat2 with BENEATH, NO_SYMLINKS and NO_XDEV; unsupported kernels fail closed,
without a pathname fallback. Symlinks, nested mounts, path traversal and hard-linked
regular files are denied. This prevents a checked path being replaced with a link
to outside files before the read. Root renames do not redirect an existing handle.

Repository .gitignore files are read through the same handle and parsed with the
ignore crate, including nested files. Invalid/unreadable/symlinked ignore files
stop the operation. Global Git excludes and .git/info/exclude are not implemented.
Dot paths, vendor/node_modules, credentials/secrets directories and key/certificate
file suffixes are denied independently of ignore rules. This is conservative, not
perfect secret recognition: non-dot files can still contain private data. Outbound
model disclosure needs context review and the selected provider policy.

Reads accept UTF-8 regular text files up to 256 KiB, reject binary/NUL data and
report errors rather than pretending an excluded file was absent. Literal search
is bounded to 256-byte single-line queries, 5000 directory entries, depth 8,
roughly 8 MiB scanned text, 100 hits and a 2-second review window. Returned lines
are UTF-8-safe excerpts up to 1024 bytes. Results include a truncation flag and
skipped-file count; skipped files are not proof of no matches. Limits apply between
IO operations; a hung filesystem can still stall a read, so production loop wiring
must run this off the UI thread and provide cancellation/timeouts.

No macOS/Windows executor is claimed. No UI trust/permission manager, approvals,
paid provider, writes, commands, test execution or edit application is enabled by
this module. Milestone 4 will add revision-checked proposals/checkpoints separately.

Verification: five read/search tests cover repository/nested ignore rules, pinned
root behavior, symlink/hardlink/secret paths, invalid ignore symlink, UTF-8 search
line numbers, binary/size/query/hit bounds. Full cargo check passed. Host gates
still own landing and complete app tests/builds. GUI verification remains separate.
