# Chat sessions and Stop (agent foundation 1)

The AI Chat composer accepts multiple lines. Enter inserts a newline; Send submits
it. Stop cancels the local worker or aborts the in-flight Ollama HTTP future. The
server may continue inference after a client disconnect; Stop cannot promise to
kill a remote server's computation. Late events belong to an invalidated run and
cannot update a newer chat.

Settings > AI Chat > Save History is off by default. Turn it on to store the
prompts, displayed context labels and replies on this device under the editor's
local data directory, separated by a hash of the workspace identity. Raw attached
file context is not stored by this feature, but prompts and model replies can
contain private code or secrets. Do not enable it if that is unwanted. Turning
it off stops new saves; it does not delete older history files.

Opening a workspace with Save History on restores its latest saved conversation
into a new session. A run that was still running at shutdown becomes Interrupted;
it does not restart inference or tools. Incomplete turns are not reused as model
history. New chat preserves the old session and starts an empty one. No history
browser or deletion UI is included yet; saved sessions are plain JSON files that
can be deleted from the local chat-sessions directory while the editor is closed.

Limits: 200 displayed turns and 2 MB per session. Storage errors remain visible
and do not delete the old file. Session writes use private files and atomic rename
on Linux. Windows rename-over-existing behavior still requires host/platform
verification. Local storage does not follow a remote workspace to its proxy.
History storage is not encrypted and has no automatic retention cleanup yet.

This is still answer-only chat, not an autonomous agent. Hosted API providers,
spend enforcement, tools, edits and checkpoints are later milestones. No hosted
request or paid model fallback is introduced here.

Verification: session and transport regression tests cover restart recovery,
workspace partitioning, invalid IDs/roles, limits, corrupt files, file symlinks,
run-generation rejection, HTTP cancellation and non-streaming responses. Full
app build and interactive multiline/Stop/restoration need the host gates and a
real Linux GUI check; standalone unit harnesses do not establish GUI correctness.
