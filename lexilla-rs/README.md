# Rust + Lexilla spike

No Scintilla widget, repository publication, editor fork, account use, or deployment.

## Run

Requirements: Rust/Cargo, a C++17 compiler (C++20 for upstream fixture runner), Linux tested.

```sh
cargo build --release --locked
cargo run --release --locked --bin lexilla-spike
cargo run --release --locked --bin lexilla-spike -- list
cargo run --release --locked --bin ts_bench
bash verification/run.sh
```

Run from this folder. `lexilla-spike` generates the 12MiB C++ input used by `ts_bench`.
Benchmark output appends to `output/benchmark.txt`. The vendored snapshots and Cargo.lock
make the source build reproducible without depending on Git branch heads.

- `shim.cpp`: document implementation and C ABI.
- `build.rs`: cc builds every lexer, lexlib, catalogue and shim into a static archive.
- `src/main.rs`: Rust RAII, sample run dump, partial relex check, measurements.
- `src/bin/ts_bench.rs`: fresh-tree C++ parse timing, no highlight query.
- `verification/TestAdapter.cxx`: upstream TestLexers runner with TestDocument replaced
  by the custom Document. Upstream TestDocument.cxx is linked only for its UTF conversion
  helper. Fixture runs actually use our adapter, not upstream's document implementation.
- `verification/edge.cpp`: line endings, UTF-8 navigation, truncated UTF-8, empty document,
  unknown lexer; tested with adapter address/undefined-behavior sanitizers.
- `output/`: raw measured evidence; large.cpp is regenerated rather than packaged.
- `config/theme-map.toml`: proposed mapping schema, not native Lapce configuration.
- `REPORT.md`: conclusions, scope, caveats and estimates.

Only selected keyword lists are configured in the small demos. The upstream fixture
runner applies each fixture's properties, keyword lists and substyles itself.

The Rust wrapper is a demo binary, not a public safe library API. Production should
validate all lengths, own/configure persistent per-buffer sessions, return typed errors,
enforce single-threaded session access and audit every C ABI boundary.

Vendored Lexilla and Scintilla licenses remain in their original folders. The source
archive omits Git metadata, build artifacts, toolchains and generated large benchmark input.
