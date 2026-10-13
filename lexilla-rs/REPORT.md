# Rust embedding of Lexilla: working spike
13 October 2026

## Decision

**GO for Lexilla as an alternative syntax-highlighting engine.** The core shim is clean,
not ugly: 106 lines of C++ build and run without a Scintilla widget. Rust's `cc` crate
compiles the full library, and Rust receives byte-indexed styles and coalesces them into
runs. The estimate of a C++ shim plus per-language style/theme mapping is technically
sound, but incomplete as a production-editor estimate. Persistent document state,
editor invalidation, configuration, and features previously using syntax trees need work.

This is a working scratch prototype, not an integrated Lapce fork or a finished safe crate.

## Exact build

| Component | Tested version |
|---|---|
| Lexilla | `dd15a115f7a9bb2bd8db1d58b24d04327b77b980`, upstream commit dated 2026-10-11 |
| Scintilla headers | `c4f161912f4afff81b0697f52f78ad7f0620ac25`, public mirror, dated 2024-09-24 |
| Rust / Cargo | 1.99.0 |
| C++ compiler | GCC 11.4.0, C++17 for library/shim; C++20 for upstream tests |
| cc crate | 1.8.0, Cargo.lock included |
| tree-sitter / C++ grammar | 0.27.1 / 0.23.4, Cargo.lock included |
| Host | Linux x86-64 container, Intel Xeon reported at 2.60GHz, 1.9GiB visible RAM |

`build.rs` compiles `src/Lexilla.cxx`, every `lexlib/*.cxx`, every `lexers/*.cxx`, and
`shim.cpp`, with `LEXILLA_NO_EXPORT`, C++17 and optimization level 3. No dynamic
Lexilla loading. Runtime enumeration returns **139 lexers**. The static archive is about
4.9MiB; the release demonstration executable about 3.4MiB, not stripped.

Both dependency snapshots are included in the source bundle. The mirror headers are
older than Lexilla but compiled successfully and passed the tests below. Production
should pin a tested current Scintilla release rather than assume arbitrary ABI compatibility.

## What is working

1. C++ source (`cpp`) produces comments, preprocessor, keywords, numbers, strings,
   characters, identifiers and operators.
2. Python (`python`) produces comments, keywords, identifiers, triple-quoted strings,
   f-string parts, built-in names, numbers and operators.
3. HTML (`hypertext`) correctly switches between HTML, PHP and embedded JavaScript.
   PHP is not a separately created `php` lexer in this spike.
4. All samples include non-ASCII text (`café` and an emoji). Ranges are **UTF-8 byte
   offsets**, half-open `[start,end)`, not Unicode character offsets.
5. A suffix relex starting at a line boundary, keeping styles and state, equals the full
   result for each sample. This is not an edit-insertion/deletion correctness test.
6. A stronger test substitutes this custom `Document` into upstream's TestLexers
   runner. **All 215 upstream fixture files pass** with exit status 0. The runner compares
   expected styles and folds and tests line-by-line and LF/CRLF variants where enabled.
   The three requested-language directories account for 42 fixtures; the full suite spans
   59 top-level directories. This does **not** mean every one of the 139 lexers is covered.
7. Adapter-only address/undefined-behavior sanitizer checks pass for CR/LF/CRLF line
   boundaries, UTF-8 navigation, truncated UTF-8, empty input and unknown lexer. The
   optimized upstream archive was not sanitizer-instrumented.

Raw output: `output/demo.txt`, the three `*-runs.txt` files, `cpp-styles.tsv`,
`upstream-tests.txt`, `all-upstream-tests.txt`, `benchmark.txt`, `tree-sitter.txt`.

## IDocument complexity

`IDocument` has **24 virtual methods** at the pinned header version. The adapter provides
all 24, as required to instantiate a concrete C++ object:

- **21/24 (87.5%) have implementations or correct UTF-8-only responses.** These include
  byte access, styles, line indexes, levels, line state, indentation, UTF-8 navigation,
  buffer access, error recording, version and encoding queries.
- **3/24 (12.5%) deliberately have no effect:** `DecorationSetCurrentIndicator`,
  `DecorationFillRange`, `ChangeLexerState`. Decorations are not syntax styles. The
  Python sample actually calls both decoration methods, so the omission is real, not
  hypothetical. `ChangeLexerState` needs an invalidation callback in an editor.
- **17/24 (70.8%) are exercised by the three small Lex+Fold samples.** This includes
  two omitted decoration methods. Thus **15/24 (62.5%) substantive methods** are
  exercised on those samples. Do not read that as the minimum for every lexer.
- `CodePage` returns 65001; `IsDBCSLeadByte` always returns false. This is correct for
  the chosen UTF-8-only contract, not general legacy-encoding support.
- `SetStyleFor`, `GetLineState`, error reporting, buffer access and indentation are
  implemented even though the small samples do not exercise them.

Storage is simple: owned text, one style byte per text byte, line-start vector, line-state
vector, line-level vector. Most complexity is correct line/UTF-8 boundaries rather than
FFI. Upstream already has a standalone TestDocument, which confirmed that a GUI widget
is not required; this spike implements its own smaller adapter and runs upstream tests
against that adapter.

### Production omissions

The demo owns a flat snapshot; it has no text editing API, rope-backed access, line-state
migration after edits, viewport scheduling, stale-result rejection or editor notifications.
It retains an ILexer instance for repeated lexing but recreates it between example sessions.
The Rust wrapper is a private demo, not a reviewed public safe API. Do not expose its raw
pointer/length assumptions without validation and an FFI audit. Ensure no C++ exception
crosses the ABI, no concurrent access to a session, and release lexers through `Release`.

## Performance

Repeated valid sample code to a minimum of **12MiB**, with a warm lex then five measured
whole-document Lex calls. Median reported below. Folding and run extraction excluded
from the Lex time. Document construction includes text copy, styles and line indexes.
The call counters are still enabled, so these are not a best-case uninstrumented build.

| Lexer | Bytes | Lex median | Throughput | Snapshot setup | Rust run extraction |
|---|---:|---:|---:|---:|---:|
| C++ | 12,583,050 | 0.182893s | 65.61MiB/s | 0.028559s | 0.053443s |
| Python | 12,582,942 | 0.170313s | 70.46MiB/s | 0.025808s | 0.047846s |
| HTML + PHP + JS | 12,583,048 | 0.240667s | 49.86MiB/s | 0.026814s | 0.049337s |

The examples create 3.3-3.8 million style runs at this size. Rust's `(usize,usize,u8)`
tuples occupy 24 bytes on this host, before vector spare capacity. C++ run storage alone
is roughly 87MiB. A real editor should not materialize every run for every large buffer
on every keypress. Keep byte styles/compact runs and produce viewport spans lazily.

Same C++ input, tree-sitter fresh full parse with no previous tree, warm pass then three
measurements: median **1.524461s, 7.87MiB/s**. Lexilla's full style pass is **8.3x less
elapsed time in this experiment**. Tree-sitter peak child RSS was 401,680KiB.

**This is not an apples-to-apples engine ranking.** Tree-sitter builds an AST, Lexilla
writes style bytes. No tree-sitter highlighting query was run. No incremental edit
benchmark, editor rendering, folding benchmark, representative corpus, pathological
long-line test or cross-platform benchmark was done. Both input and grammar choices
matter. These numbers support feasibility, not a claim that Lapce will become 8x faster.

## Theme mapping: concrete shape

`NamedStyles`, `NameOfStyle` and `TagsOfStyle` provide runtime metadata. C++ returns a
style space of 92 entries, with normal definitions 0-27, a reserved gap 28-63, and inactive
variants 64-91. Inactive entries have tags but no names. Selected actual outputs:

| ID | Runtime name | Tags | Proposed theme token |
|---:|---|---|---|
| 1 | SCE_C_COMMENT | comment | comment |
| 2 | SCE_C_COMMENTLINE | comment line | comment |
| 4 | SCE_C_NUMBER | literal numeric | number |
| 5 | SCE_C_WORD | keyword | keyword |
| 6 | SCE_C_STRING | literal string | string |
| 9 | SCE_C_PREPROCESSOR | preprocessor | keyword |
| 10 | SCE_C_OPERATOR | operator | operator |
| 11 | SCE_C_IDENTIFIER | identifier | default |
| 19 | SCE_C_GLOBALCLASS | identifier | type |
| 27 | SCE_C_ESCAPESEQUENCE | literal string escapesequence | string.escape |
| 69 | empty | inactive keyword | keyword with reduced opacity |

Example from the included **proposed schema**, not a native Lapce config:

```toml
[lexers.cpp.styles]
"1" = "comment"
"5" = "keyword"
"19" = "type"
[lexers.cpp.secondary]
base_offset = 64
opacity = 0.55
```

IDs are scoped to the lexer: style 5 cannot be assumed to mean the same thing everywhere.
A generic tag-to-token fallback can cover many styles, reducing 139 fully handwritten
maps to generated defaults plus overrides. The `GLOBALCLASS` entry shows why tags alone
are insufficient: it says identifier, but the intended color may be type. PHP/script
styles under hypertext need their own overrides. Reserved IDs, inactive variants and
user substyles need deliberate handling. Keyword lists and lexer properties are also
part of highlighting, not merely theme colors.

## Revised estimate: engineering judgment, not measured project delivery

Assumptions: one experienced Rust/C++ editor engineer, an existing Lapce-style buffer and
render pipeline, syntax highlighting first, desktop Linux/macOS/Windows, reuse upstream
fixture/config data where legal. No claim that all features previously driven by syntax
trees can be removed for free.

| Work | Estimated effort |
|---|---|
| Harden adapter, safe API, error handling, dependency pins, cross-platform builds | 3-5 engineer-days |
| Persistent buffer integration, line state/fold state, edit invalidation and UI spans | 5-10 engineer-days |
| All 139 registered lexers wired; generated metadata/default maps and configuration plumbing | 5-10 engineer-days |
| Curated per-language keyword/property/theme parity, gaps in upstream tests, edit/performance regression tests | 10-25 engineer-days |

Allow **roughly 5-10 engineer-weeks** for a reviewed, cross-platform, broadly usable
highlighting integration, with a first limited-language editor milestone in **1-2 weeks**.
The ranges overlap and are planning estimates, not a fixed bid. Literal visual/behavior
parity for every Notepad++ language can take longer and requires inspecting Notepad++'s
actual bundled lexer revision, keyword/config files, themes and possible patches. This
spike pins ScintillaOrg Lexilla, not Notepad++'s distribution. No 139-language parity claim
has been established.

Replacing tree-sitter wholesale is a different project. Structural selection, syntax-aware
motions, indentation, injections, search and other tree consumers would need alternatives
or retained tree-sitter. Lexilla supplies highlighting and folding, not a general syntax AST.
LSP semantic tokens can coexist, but precedence rules must be defined and tested.

## Next acceptance gate

Integrate only C++, Python and HTML/PHP into a real buffer first. Test random insert/delete
sequences against full lexing, multibyte edits, CRLF, tabs, malformed input, 10MiB buffers
and long lines; keep editor revision IDs and reject stale background results. Profile
viewport run generation and demonstrate cross-platform builds. Then extend language
configuration and mapping across the catalogue. Keep tree-sitter for non-highlighting
features unless there is a separate migration plan.

## Sources and provenance

Primary findings above are measured by the included source and captured output, not
external benchmark claims. Public sources used for build/interface semantics:

- https://github.com/ScintillaOrg/lexilla : actual cloned source, pinned commit above;
  README, Lexilla.cxx, lexer metadata and TestLexers fixture runner inspected locally.
- https://www.scintilla.org/LexillaDoc.html : fetched official documentation, last edited
  24 May 2026; static linking, CreateLexer, no GUI dependency, LEXILLA_NO_EXPORT.
- https://github.com/mirror/scintilla : actual cloned source, pinned commit above;
  include/ILexer.h is the source for the 24-method interface count.

Both source distributions carry the Lexilla/Scintilla license permitting use, copy,
modification and distribution with the copyright and permission notice retained. Notices
are included in the bundle. This is not a legal review of Notepad++ or Lapce distribution.
