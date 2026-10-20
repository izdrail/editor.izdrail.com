# Composer send keys (item 281)

Enter sends through the existing send path. Shift+Enter uses the editor's
newline handler. The current composer document is synchronized before send.
When mention suggestions are open, Enter still selects the highlighted mention
first; a subsequent Enter sends. Busy/disabled/empty guards remain unchanged.
No automatic retry or change to remote/local permission checks.

Host: type dummy prompt, Enter produces one user turn and clears input;
Shift+Enter adds newline without a request; mention Enter inserts mention, then
Enter sends; busy Enter does not duplicate a request. Test fixture local/remote
profiles, normal/narrow composer and focused terminal (must not send chat).
