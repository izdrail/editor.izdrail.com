# AI commit messages

The source control panel has a "Suggest message (AI)" button next to Commit
when AI Chat is enabled with the ollama backend (the default
https://ai.izdrail.com). It asks the configured chat model for one
conventional-commit style message over the checked files' unified diff and
replaces the commit editor's content with the reply. The user edits or
commits as usual; nothing is committed automatically.

Flow: the panel sends the checked file list to the proxy, which builds a
HEAD-to-worktree+index patch with git2 (untracked files included, renames
pathed both ways), hard-capped at 256 KB with an explicit truncation marker.
The app trims that to a 12 KB prompt budget on a char boundary, again with a
marker. Only the diff text goes to the endpoint - the same network posture
as AI Chat (no keys, no file contents beyond the diff, no telemetry).
Clicking the button again while a suggestion is in flight cancels the
request. An empty diff, a backend error or a cancelled request leaves the
commit editor untouched.

Settings: the lane uses the AI Chat endpoint and model; there is no
separate configuration. Disable AI Chat or switch its backend to local to
turn the button off (it hides itself).

Verification: prompt shaping, truncation char-boundary safety and marker
behaviour are unit tested (no network). The git2 patch builder and the
button wiring are UNVERIFIED locally - host gate and pixel QA are
authoritative.
