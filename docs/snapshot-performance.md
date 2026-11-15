# Large-file snapshot performance

Rank-11 plan gate: *measured large-file latency/memory*. This doc records
what a full `Syntax::parse` snapshot costs on large PHP/HTML files: the
Lexilla highlight snapshot, the Fold pass that produces fold levels
(item 446) and folding-range derivation. Snapshots relex the full source
per accepted revision, so this measurement is the responsiveness ceiling
for opening or editing a large file.

## How to run

```
cargo run --release -p lapce-core --example snapshot_perf
```

The harness is `lapce-core/examples/snapshot_perf.rs`. It is an example,
not a test: the suite and CI do not run it, so it adds no CI time. It
builds deterministic generated corpora (no fixture files, no randomness),
times full parses, and reads `/proc/self/status` for RSS. `--only N`
runs one case in a fresh process for a clean memory delta; the doc table
below uses per-case fresh processes.

Corpus shapes (generated, representative, not pathological):

- **pure php**: namespaced classes with methods, foreach/if nesting,
  string interpolation, heredoc and comments.
- **mixed**: HTML markup with interleaved `<?php if/foreach/else ?>`
  islands, the common theme-template shape.
- **html**: nested structural tags (`div/article/header/footer`), the
  tag-folder case.

## Results (2026-11-15, build host, release profile, commit 5378d8e)

| corpus          | lines   | median latency | per MiB | fold ranges | retained RSS delta |
|-----------------|---------|----------------|---------|-------------|--------------------|
| pure php 1 MiB  | 42,601  | 20 ms          | 20 ms   | 6,816       | +23.9 MiB          |
| pure php 5 MiB  | 211,726 | 118 ms         | 24 ms   | 33,876      | +34.2 MiB          |
| mixed    1 MiB  | 43,325  | 38 ms          | 38 ms   | 7,222       | +9.3 MiB           |
| mixed    5 MiB  | 214,085 | 187 ms         | 37 ms   | 35,682      | +43.4 MiB          |
| html     1 MiB  | 34,011  | 33 ms          | 33 ms   | 9,718       | +10.1 MiB          |
| html     5 MiB  | 167,074 | 182 ms         | 36 ms   | 47,736      | +42.3 MiB          |

Retained RSS delta is one live snapshot plus its rope, measured in a
fresh process (`--only`). Small-file deltas include one-off allocator
growth, so the 5 MiB rows are the reliable read: roughly 7-9 bytes of
retained memory per source byte.

## Interpretation

- Latency is linear in file size: 20-37 ms/MiB across lexers, no
  cliff at 5 MiB. A full 5 MiB snapshot lands in ~120-190 ms, inside
  interactive tolerance for a one-shot open and batched revisions.
- The item-446 Fold pass (levels + `level_ranges`) is included in every
  number above; leaving it unconditional costs nothing measurable beyond
  the highlight pass it rides on.
- Memory is ~7-9x source size for a retained large-file snapshot.
  Acceptable, but worth knowing before holding many large snapshots;
  cancelled superseded jobs already exit before lexing.
- No incremental relex is warranted at these sizes; the full-snapshot
  design stays. Revisit only if real-world files past ~10 MiB become
  common editing targets.
