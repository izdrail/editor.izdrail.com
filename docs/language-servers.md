# Independent LSP and server registry

Base: editor.izdrail.com main efc6f3d. No PSP/catalog/WASI restoration. Lexilla
highlighting and the data-only themes/icons marketplace are unchanged.

Restores completion + completion resolve, hover, go-to-definition and pushed
diagnostics through the existing app/RPC UI. Uses stdio Content-Length JSON-RPC,
UTF-16 positions, initialize/initialized, didOpen, versioned didChange, didSave,
didClose and bounded shutdown/reaping. Incremental servers receive a full-document
replacement with the correct old UTF-16 range, not an invalid un-ranged Full change.
Request timeout is 20 seconds; failed-server start retries have a 30-second backoff.
Dead servers reopen retained documents on the next action. Closing the last editor
for a file sends didClose, and versioned stale diagnostics are ignored by the app.

## Registry

Global `language-servers.toml` is created in the editor config directory on first
proxy initialization. Project settings cannot provide commands or npm install
specifications. Restart the workspace after editing this file. Config:

```toml
enabled = true
auto-install = true

[servers.custom]
languages = ["php"]
command = "/path/to/server"
args = ["--stdio"]
enabled = true
initialization-options = {}
packages = []
```

The seeded registry covers npm-managed TypeScript/JavaScript, Python (Pyright),
HTML, CSS/SCSS/Less and JSON/JSONC. Rust (rust-analyzer), Go (gopls) and PHP
(phpactor language-server) are command-managed and must already be installed.
An entry for the stock `phpactor` command that omits `args` is backfilled to
`["language-server"]` on load: a bare `phpactor` invocation prints usage and
exits instead of serving LSP (item 384). Custom commands keep their own args.
Custom commands/languages/options are supported. If multiple entries claim a
language, the enabled alphabetically-first server wins; disable duplicates.
It is one server per language, not multi-server aggregation.

Managed npm packages have pinned versions. Install location is the editor data
folder's language-servers/<server>/, never the project. Missing or changed pinned
packages install lazily on first use. Install uses --ignore-scripts, --engine-strict,
no audit/fund, a 180-second timeout, staging/rollback and a per-server lock.
Package list fingerprints cause version changes to reinstall on next workspace
restart. Remove a server directory manually to uninstall managed binaries.
No server install/update UI, universal binary downloader, signature verification,
background update watcher or installed-system-server updater is claimed.
Node.js/npm must exist on PATH; seeded TypeScript server requires Node >=22.22.2.
Managed npm launch is Linux/macOS only in this version. Windows needs direct
commands such as node.exe plus the server script path; npm .cmd shims are not
silently executed through a shell. Remote workspaces install/run on the proxy host.

## Execution boundary

Language servers are native third-party programs, not data packages. They may
read/index the workspace and some can execute project code. This task deliberately
reintroduces that opt-in product capability. Disable the registry (`enabled=false`)
to prevent launch, or disable auto-install to require already installed servers.
The global registry is trusted user configuration; do not populate it from
untrusted repositories. npm lifecycle scripts are suppressed, but server code and
npm dependencies are still executable. No WASM/Volt execution path is reopened.

The manager serializes lifecycle/document changes/requests, keeping proxy file IO
responsive but allowing one slow server request/install to delay other language
service work. No request coalescing, per-server parallel workers or user cancel UI
is implemented. Workspace/applyEdit and arbitrary process/debugger host requests
are rejected; dynamic capability registration, semantic tokens, symbols, rename,
formatting, references, code actions, pull diagnostics and advanced IDE features
remain unsupported, not falsely advertised. Existing local folding is unchanged.

Installation is not crash-atomic: rename rollback preserves old state on normal
errors, but an unexpected process crash may leave a hidden backup/stale lock.
Registry packages are fetched from npm, not the Microsoft marketplace. The
editor's themes/icons installer remains a separate data-only trust boundary.

## Verified

- Workspace cargo check passes.
- Proxy lib suite: 24 pass, 3 explicit network/real-server tests ignored.
- Core/RPC and non-app workspace suites/doctests pass.
- App lib test executable: 23 pass, 1 explicit real-theme smoke ignored. It was
  built with `cargo rustc -p lapce-app --lib --profile test -- -C link-arg=-Wl,--threads=1`
  to avoid the default link resource limit, then executed directly.
- Mock end-to-end dispatcher completion/hover/definition/diagnostics test passes.
- Explicit pinned npm auto-install of Pyright into a temporary directory passes.
- Explicit real Pyright completion, hover, definition and diagnostics receipt passes.
- No installations were made into the user's profile for tests.

Final ordinary GUI binary build and interactive/visual smoke test are NOT verified
in this workspace: the normal app build hit the runtime limit/filesystem I/O stall.
The host lane must run the full ordinary build/test gate and GUI smoke before
shipping. All tests above refer to Linux. macOS/Windows behavior needs testing.

Sources inspected:
- https://github.com/izdrail/editor.izdrail.com (main efc6f3d)
- Old fork parent of 4b0dce2, lsp.rs/psp.rs/catalog coupling inspected locally.
- https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/
- https://github.com/microsoft/pyright
- https://github.com/typescript-language-server/typescript-language-server
- https://github.com/hrsh7th/vscode-langservers-extracted
- https://registry.npmjs.org/pyright/latest
- https://registry.npmjs.org/typescript-language-server/latest
- https://registry.npmjs.org/typescript/latest
- https://registry.npmjs.org/vscode-langservers-extracted/latest
- Existing private research/vscode-extensions-lapce/README.md and
  04-options-ranked.md retrieved from https://github.com/izdrail/research.izdrail.com.

## PHP (phpactor)

The stock `php` entry runs `phpactor language-server`. If `phpactor` is on PATH it
is used as is. If not, and `php` is on PATH, the editor downloads the pinned
phpactor 2026.10.07.0 phar from the phpactor GitHub release into
`language-servers/phpactor/` under the editor data directory, checks its SHA-256
(`b28bf616...bdb16b`), and runs `php phpactor.phar language-server`. A phar that
fails the hash check is never run. `auto-install = false` disables the download.
With no `php` binary the server is skipped quietly (one log line, no popup).
Setting `command` to anything other than `phpactor` (for example an absolute path)
keeps your own server, with no fallback. Not verified: phpactor's PHP version and
extension requirements on a given machine.

## Laravel (laravel-lsp)

The seeded, disabled `laravel-lsp` entry is the official Laravel language server
(laravel/lsp, MIT): completions, hover, diagnostics, document links, definitions
and code actions for routes, views and Blade, translations, config, env,
middleware, Inertia, Livewire, policies, container bindings, validation rules,
controller actions and Eloquent completions - the framework awareness phpactor
does not have. It is one server per language, so it replaces phpactor for
`php`/`blade` when enabled; it does not run alongside it.

To switch, edit the global `language-servers.toml` and restart the workspace:

```toml
[servers.laravel-lsp]
languages = ["php", "blade"]
command = "laravel-lsp"
args = []
enabled = true
requires-laravel = true
initialization-options = { phpEnvironment = "auto", pestGenerateDocBlocks = false }
packages = []

[servers.php]
# ... keep the rest of the phpactor entry, but disable it:
enabled = false
```

If both entries are enabled, `laravel-lsp` (alphabetically first) serves PHP and
Blade inside Laravel projects and phpactor still serves them everywhere else -
the `requires-laravel` gate falls through to the next enabled entry, so this
combo is a valid "laravel-lsp where possible, phpactor otherwise" setup.

Gating reuses the static LexLaravel project detection: an `artisan` file or a
`composer.json` requiring `laravel/framework` at the nearest project root above
the workspace. The server launches with that detected root as its working
directory and rootUri, not a parent folder. The probe result is cached for 60
seconds. Outside a Laravel project the server is never downloaded or started.

If `laravel-lsp` is on PATH (for example from `composer global require
laravel/lsp`) it is used as is. Otherwise the editor downloads the pinned
v0.0.32 self-contained binary for the current platform (x64/arm64 Linux, x64/
arm64 macOS, x64 Windows) from the laravel/lsp GitHub release into
`language-servers/laravel-lsp/` under the editor data directory, checks its
SHA-256, marks it executable and runs it with no arguments (its default command
is `lsp`, stdio). A binary that fails the hash check is never run.
`auto-install = false` disables the download. The binary bundles its own PHP
runtime, so unlike phpactor no `php` is needed to launch the server itself.

Defaults sent in `initializationOptions`: `phpEnvironment: "auto"` (the server
picks herd/valet/sail/lando/ddev/local itself) and `pestGenerateDocBlocks:
false` (that feature writes helper files under `storage/`). The server resolves
`phpCommand` to `["php"]` on its own; override either key in the entry to change
them. Feature requests shell out to the project's own PHP, so routes/config/
Eloquent awareness needs `php` plus `vendor/` installed in the project; the
initialize handshake itself needs neither. Verified against v0.0.32: initialize
over stdio returns completion/definition/hover/codeAction/documentLink and sync
capabilities, logs INFO to stderr. The release is young (v0.0.x) and pinned on
purpose; bump the version and all five digests together after checking a new
release.
