# Lexilla highlighting milestone

The editor links the seeded, pinned Lexilla catalogue through `lexilla-rs`.
`Syntax::parse` uses a safe UTF-8 snapshot interface, no tree-sitter parser or
runtime grammar downloads. The worker publishes only if its cancellation flag
is clear, and Doc checks the buffer revision before accepting/painting it.
Edits invalidate old highlights while the replacement snapshot is prepared.

Byte styles are kept compact (one byte per source byte). Runs and theme scopes
are produced only for a requested painted line in `Doc::line_style`. Multiline
comments and embedded languages still see the whole document context. No
full-file run vector or millions of rope style spans are built.

The shim serializes first catalogue initialization with C++ call_once because
upstream lazy initialization is not thread-safe. A parallel-first-use regression
test covers this; sessions remain independent afterward.

Initial configured modes: C, C++, JavaScript, TypeScript, C#, Java, Python,
HTML, PHP, CSS, JSON, JSON5, Bash, Rust, SQL, TOML, YAML and XML. The registry is
in `lapce-core/src/lexilla.rs`; other modes currently show plain text. The
native archive includes the full 139-lexer catalogue, not full configuration
coverage. Style metadata tags are mapped to existing Lapce theme scopes;
unknown/default/identifier styles keep the normal foreground. Keyword lists
are initial sets, not exact Notepad++ definitions. Java/C#/JS currently use the
shared C-family lexer. Per-language Notepad++ overrides remain future work.

This milestone deliberately relexes a fresh snapshot after an edit. That avoids
unproven state migration and stale multiline styling, but costs O(document
bytes) per edit and duplicates source text during lexing. Native lexing cannot
be interrupted mid-call yet; stale/cancelled results are discarded afterward.
There is no large-file typing responsiveness claim. Incremental edit handling,
coalescing work and memory/performance gates remain needed. Tree-sitter source
helpers still compile but do not run in the editor highlighting path. Tree-based
sticky headers, structural navigation and syntax lens are unavailable.

Headless verification (no UI screenshot claimed):

```sh
cargo test --manifest-path lexilla-rs/Cargo.toml --lib --locked
cargo test -p lapce-core lexilla --lib --locked
(cd lexilla-rs && bash verification/run.sh)
cargo build --workspace --locked
cargo test --workspace --locked
```

The snapshot tests reconstruct viewport bytes and compare them with full styles
for C++, Python and HTML/PHP/JS, including CRLF and multibyte Unicode. Edit/undo
tests verify fresh multiline state. Core tests check real theme conversion,
configured lexer availability, Syntax worker integration and cancellation.
The upstream 215-fixture adapter suite remains reproducible in `verification/`.

For a UI smoke check, open samples/example.cpp, example.py and example.php;
scroll, edit a multiline comment delimiter, undo, change language and theme.
Confirm scopes recolor, stale snapshots never show after edits, and no grammar
or language-server downloads/processes occur. This still needs pixel inspection
on a display-enabled build machine before claiming the UI smoke gate passed.
