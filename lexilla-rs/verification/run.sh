#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --locked
archive=$(find target/release/build/lexilla-rs-*/out -name liblexilla_static.a | head -1)
g++ -std=c++20 -O2 -DLEXILLA_STATIC -Ivendor/scintilla/include -Ivendor/lexilla/include -Ivendor/lexilla/access verification/TestAdapter.cxx vendor/lexilla/test/TestDocument.cxx vendor/lexilla/access/LexillaAccess.cxx "$archive" -ldl -o verification/test-adapter
(cd vendor/lexilla && ../../verification/test-adapter)
