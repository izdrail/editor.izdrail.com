# Candle FIM editor patch: verification and handoff

13 October 2026. Experimental, not release-ready.

## Delivered

One editor format-patch against live main `e0648512e19d177a3122cd300e01fb52c1b9963f`, authored Stefan Bogdanescu <bogdan.izdrail@gmail.com>. Research new-file patch was delivered earlier and is already present identically in the supplied research snapshot (real research main 0d2e5a2). Do not apply the snapshot's synthetic root commit.

- New lapce-ai crate uses Candle 0.9.1 quantized Qwen2 architecture. Our code owns FIM token construction, context/output budgets, greedy sampling, stops, integrity/download handling and worker lifecycle. This is not a new kernel engine or a separately rewritten transformer.
- Provider trait leaves room for an HTTP implementation later, none implemented now.
- Persistent child worker lazily loads CPU weights, processes bounded requests, and is hard-killed on cancellation/edit. It runs outside UI and proxy dispatcher. Cold reload is the cancellation tradeoff. No idle or memory-pressure unloading.
- Manual inline RPC now enabled; generation, document ID, revision, cursor and enabled-state guards reject stale replies. Inherited keybinding is Alt+Backslash in Insert mode. Automatic hooks intentionally do not infer. Inherited accept/cancel bindings remain.
- New AI Autocomplete settings: enabled=false, model-path, tokenizer-path, prefix-tokens=512, suffix-tokens=128, max-tokens=32. Limits 1..2048, 0..1024 and 1..128 respectively. New toggle enables rendering without requiring the old editor inline toggle.
- Explicit model download/progress/cancel controls in Settings. Curated QuantFactory 1.5B BASE Q4_K_M plus official tokenizer, immutable revisions and independent size/SHA-256 validation. Stored under local editor data ai-models/hub. Installed manifest only written after both validate. No source code upload, implicit fetch or update.
- docs/ai-autocomplete.md documents use, source/licence, limits and gaps.

## Tests actually run

- Isolated engine crate: 7 passing tests, 1 real-model test ignored in normal run. Tests cover real-tokenizer Unicode/budget prompt construction, greedy tie/NaN behaviour, stop/max limit, cancellation, suffix-overlap clipping, checksum rejection and subprocess successful response/hard cancellation.
- Real 0.5B Q4_K_M optimized integration test: Rust and PHP both pass. Rust output `a + b`; PHP `return $a + $b;`. The model generated past the hole in Rust, so overlap clipping removes the existing suffix. This still evaluates more tokens than useful; it is not a broad quality benchmark.
- Whole-workspace cargo check passed after rebasing to e064851. Local verification used CARGO_BUILD_JOBS=1, local extracted OpenSSL dev files and a temporary Lexilla build-script O0 override to speed native compilation. That override was restored and is NOT in the patch. Original O3 native build was attempted but repeatedly ran beyond bounded command windows; do not claim an unchanged native release build was completed. Existing editor warnings remain.
- Standalone optimized lapce-ai binary builds and executes.
- Actual hf-hub downloader succeeded: approximately 1 GB transferred, progress/verifying/ready events, both expected hashes passed and installed.json created. Cached retry completed in 1.411s with Ready and no transfer progress. No forced interrupted-resume or damaged-cache recovery test yet.
- Exact target 1.5B artifact downloaded and SHA verified. Real-model load test terminated SIGKILL on this workspace before completion. Cause is consistent with low memory, not independently confirmed OOM telemetry. No 1.5B performance claim.
- git diff --check passes.

## Measured CPU performance

Workspace: x86_64 Intel Xeon Processor @ 2.60 GHz; 2 exposed vCPU (one core/two threads), AVX2/AVX512 advertised, 1.9 GiB RAM, no swap. CPU-only Candle, release opt-level=3, no BLAS/GPU/native-target flags. Default library thread policy; not a controlled thread sweep. Model bartowski Qwen2.5-Coder 0.5B Q4_K_M, 397808128 bytes, revision 01c1b8072277dfbfd0d051ec95b7a43bb23d2f6a, SHA fefe9d34df79d3ad3db33519d81112be76096626a5fdb87a98092035dc79ad68. Tokenizer official Qwen 0.5B revision 8123ea2e9354afb7ffcc6c8641d1b2f5ecf18301; bytes equal the official 1.5B tokenizer hash.

Latest run, one cold load shared across both fixtures:

| Fixture | Prompt tokens | Generated tokens | Cold load | Prefill | Decode | Total generation | Decode rate |
|---|---:|---:|---:|---:|---:|---:|---:|
| Rust | 25 | 24 | 1.592s | 4.513s | 5.399s | 9.912s | 23/5.399 = 4.26 t/s |
| PHP | 18 | 7 | already loaded | 3.514s | 1.728s | 5.242s | 6/1.728 = 3.47 t/s |

Prefill produces first generated token, so decode rate uses N-1 tokens. End-to-end generated-token rates are 2.42 and 1.34 t/s. These are single-run fixture numbers, not p95 or typing latency. Both are too slow for automatic suggestions on this hardware; manual-only is intentional. No prediction about Stefan's RTX/Fedora host is justified.

## Remaining gates and caveats

1. No actual editor pixel inspection, settings layout, ghost-text display, accept/undo, Tab arbitration or rapid switch/cursor test in this headless workspace. UI readiness is unverified. Download controls currently sit near the settings search area, while editable fields are in the AI section; verify/refine this layout on the host.
2. No full editor link/release build. Host must build binaries and inspect the UI before promoting the patch.
3. 1.5B real FIM test still requires a larger-memory host. Test quality/latency with realistic context budgets. The real test uses very small prompts and does not prove production usefulness.
4. Download interrupted-resume, out-of-disk, corrupted cache, repeated rapid cancel/restart and concurrent windows need tests. hf-hub supplies resume/lock support, but integration correctness is not proven by one completed transfer. Bad cached checksum is rejected, not automatically repaired. No uninstall action.
5. Download worker cancellation during the tiny spawn window can miss the child; rapid cancel/restart should be hardened before release. Settings close does not explicitly terminate an ongoing download. Progress total is per file, not a single combined percentage.
6. Local workspaces only for this first version: downloader runs on UI host, inference follows proxy host. Remote paths/model transfer and scratch buffers are not supported consistently. No silent fallback to external inference.
7. Inference queue is bounded and hard cancellation is tested with a mock process, but UI cancellation races and worker timeout are not yet fully tested. Worker has no inference deadline. Missing model paths currently yield no ghost text; improve user-visible error notices before release.
8. Prompt budget/stop sampling uses Qwen tokenizer and architecture, not arbitrary GGUF support. User-supplied local model overrides are not signed or independently checked unless curated downloader installed them. Hash checks establish bytes, not community model quality.
9. Existing config loading permits workspace settings to set AI fields. Review trust handling for untrusted projects before shipping.

## Sources

https://huggingface.co/QuantFactory/Qwen2.5-Coder-1.5B-GGUF
https://huggingface.co/api/models/QuantFactory/Qwen2.5-Coder-1.5B-GGUF?blobs=true
https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B
https://huggingface.co/bartowski/Qwen2.5-Coder-0.5B-GGUF
https://huggingface.co/api/models/bartowski/Qwen2.5-Coder-0.5B-GGUF?blobs=true
https://huggingface.co/Qwen/Qwen2.5-Coder-0.5B
https://docs.rs/hf-hub/0.4.3/hf_hub/api/sync/index.html
https://github.com/huggingface/candle
https://github.com/izdrail/editor.izdrail.com
