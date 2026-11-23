# Jump to a referenced file

Ctrl+left-click uses the existing Go to Definition route on Linux and Windows. On macOS use Cmd+left-click. Symbol destinations still come from existing local resolvers or the language server.

Quoted exact file paths now open without a language server in a local workspace. The path must include an extension and point to an existing file inside the workspace. Relative paths use the current source file's folder first, then the workspace root. Parent segments are allowed only when the resolved destination stays inside the workspace. Symlinks outside the workspace, absolute paths, URLs and dynamic/interpolated paths are not opened by this fallback. No file search or command execution occurs.

Supported delimiters are single quotes, double quotes and backticks. Click inside the path, not the quote. Existing template-reference resolution takes precedence. Unrecognized references continue through the existing definition providers. This is an exact-path fallback, not import alias resolution, extension guessing or a guarantee that every language server has definition support.

Acceptance uses a disposable PHP source and target with language servers disabled. Inspect source and target screenshots before accepting the gesture. Unit tests cover source-folder precedence, workspace paths, parent paths, Unicode byte boundaries, malformed/dynamic strings and outside symlinks.

Rapid Ctrl-click navigation uses the exact pointer offset, even when the operating system reports a double/triple click. It resolves on the clicked editor directly instead of routing through a potentially stale globally active editor. Ordinary unmodified double/triple-click selection remains unchanged.
