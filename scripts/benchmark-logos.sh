#!/usr/bin/env bash
# Manual release benchmark; no PHP execution, dependency download or deploy.
set -euo pipefail
cd "$(dirname "$0")/.."
out=${1:-target/logos-benchmarks}
mkdir -p "$out"
cargo build --locked --release -p lapce-core --features logos-php --example snapshot_perf
binary=target/release/examples/snapshot_perf
rustc --version > "$out/environment.txt"
uname -sm >> "$out/environment.txt"
for size in 1 5; do
  "$binary" --compare --mib "$size" --iters 3 > "$out/logos-bench-release-${size}m.json"
  for backend in lexilla logos; do
    GRAVITY_LEXER=$backend SNAPSHOT_PERF_SYNTAX=$backend "$binary" --compare --mib "$size" --iters 3 > "$out/logos-syntax-$backend-${size}m.json"
    for case in php mixed blade laravel symfony; do
      SNAPSHOT_PERF_CASE=$case SNAPSHOT_PERF_RSS_BACKEND=$backend "$binary" --compare --mib "$size" --iters 1 > "$out/rss-$backend-$case-${size}m.json"
    done
  done
done
