# Vendored sources

The full vendored trees are included in this directory (about 14 MB total), so the crate builds offline with `cargo build --locked`.

| Component | Pinned commit |
|---|---|
| Lexilla (`vendor/lexilla`) | `dd15a115f7a9bb2bd8db1d58b24d04327b77b980` |
| Scintilla headers (`vendor/scintilla`) | `c4f161912f4afff81b0697f52f78ad7f0620ac25` |

Origin: the Rust-Lexilla binding spike (see `REPORT.md`, 215/215 upstream fixtures). The directory is excluded from the lapce cargo workspace (`exclude = ["lexilla-rs"]` in the root `Cargo.toml`) and builds on its own.
