# Editor without language servers (phase 1)

The executable plugin catalog, LSP/PSP clients, WASI runtime and DAP backend
are removed from the proxy crate and the dependency graph. No plugin engine is
started at workspace initialization, file open, edit or save. Existing local
color/icon theme packages are read as data only. Marketplace listing and
installation are disabled. Plugins and Debug are absent from the default panel
order; saved layouts and old commands can still contain compatibility UI.

The local proxy remains for file IO, watcher, search, git and terminal services.
This is not yet a removal of the remote workspace transport or every IDE view.
Legacy lsp-types are still used as position/range and response DTOs throughout
the app and RPC crates. Keeping DTOs does not retain a language-server client.
Language-service requests receive error -32601 immediately rather than hanging.
Language-service/plugin/debug notifications cannot launch external services.

Verification:

```sh
cargo build --workspace -j 2
cargo test --workspace -j 2
cargo tree --workspace | grep -E 'wasmtime|wasi-common|psp-types|wasi-experimental'
```

The final grep must return no matches. Focused dispatcher tests prove requests
terminate without a plugin catalog and buffers still open. UI smoke gate: start
with an existing profile containing language-server volts, open/edit/save a
file, and check no language-server or WASM plugin process is started.

Phase 2 replaces tree-sitter highlighting with the seeded Lexilla binding.
