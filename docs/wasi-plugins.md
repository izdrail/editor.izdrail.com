# Executable (WASI) plugins

Executable plugins are restored: the editor again runs Lapce-style WASI
plugins ("volts") in addition to validated data-only theme/icon packages.
Debugging (DAP) remains intentionally absent.

## What runs where

- `lapce-proxy/src/plugin/catalog.rs` - the plugin catalog. One catalog runs
  per workspace inside the proxy. It discovers installed volts, starts the
  ones whose `[activation]` matches an open file's language or a workspace
  glob, forwards document lifecycle events (open/change/save) and routes
  editor requests to plugins.
- `lapce-proxy/src/plugin/wasi.rs` - the WASI host. Each volt's WebAssembly
  module runs in wasmtime with a WASI context: its own directory preopened at
  `/`, `VOLT_*` environment variables, JSON-RPC over stdin/stdout pipes and
  the experimental HTTP host functions Lapce plugins use.
- `lapce-proxy/src/plugin/psp.rs` - the Plugin Service Protocol between the
  editor and plugins: requests (hover, completion, code actions, formatting,
  ...), host services (show message, progress, execute process, start LSP
  server) and dynamic capability registration.
- `lapce-proxy/src/plugin/lsp.rs` - language servers started *by plugins*
  (`startLspServer`) are spoken to over stdio and registered in the catalog
  as plugin servers, so plugin-started servers answer the same editor
  requests.

## Relationship with the built-in language server client

The independent stdio LSP client (registry-driven, see
[language-servers.md](language-servers.md)) is unchanged. Editor requests
(completion, hover, definition, references, code actions, document symbols,
formatting, rename, inlay hints, code lens, folding ranges, call hierarchy,
selection ranges, implementation, type definition) fan out to **both** the
built-in client and the plugin catalog; the first successful answer wins.
Completion additionally merges per-plugin results in the popup.
Plugin-provided completion items and code actions resolve through whichever
side produced them.

## Installing plugins

Executable volts install from the same panel as themes. Installation uses the
hardened data-only installer path (HTTPS-only, size-bounded, no redirects,
no links/special files, identity check); packages flagged `wasm` may contain
the declared `.wasm` module and are started after install. Disabling or
uninstalling a volt stops its running plugin.

## Trust boundary

A WASI plugin is code you run. Be plain about what it can do:

- It can make HTTP requests to **any host** (the HTTP host functions are
  configured allow-all).
- It **inherits the editor's environment variables**, plus the `VOLT_*` ones.
- It can **execute processes** on your machine through `executeProcess`, and
  start language servers.
- Only the **filesystem** is confined: the plugin sees its own directory,
  preopened at `/`.

So the sandbox limits file access, not network, environment or process
execution. Only install plugins you trust, and do not keep secrets in the
environment the editor is started from.

## Verification

```sh
cargo test -p lapce-proxy
```

`plugin::wasi::runtime_tests` drives the real catalog against the prebuilt
sample volt in `lapce-proxy/src/plugin/wasi/plugins/`: discovery, activation
on document open, guest start and a host->guest->host request roundtrip.
`plugin::wasi::tests` covers volt manifest loading; `plugin::data_only`
covers the installer, including executable packages.
