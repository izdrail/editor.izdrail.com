# Inference acceptance checkpoint, 24 November 2026

This is a checkpoint, not a completion report or backend recommendation.

| Area | Status | Evidence or remaining check |
| --- | --- | --- |
| Existing architecture audit | PASS | docs/inference-provider-audit.md, pinned source APIs inspected |
| Exact GGUF metadata | PASS, header only | target-model-header.json, qwen35moe and mixed tensor type table |
| Full target checksum | BLOCKED | full 22.36 GB file not retrieved here; owner collector verifies full file |
| Candle/Ollama preserved | PASS, automated CPU checks | hosted workspace tests and unchanged wire shapes |
| Ollama metrics/parser/pool/discovery | PASS, tests | bounds, Unicode/error frames, explicit tags/show, pool retained |
| Candle context/tokenizer checks | PASS, synthetic tests | model bounds and GGUF token-ID mapping |
| Candle CUDA code path | IMPLEMENTED, UNVERIFIED | optional feature/device selection, no CUDA build or GPU inference yet |
| llama.cpp CPU build | PASS | hosted optional native feature tests |
| llama.cpp chat/FIM implementation | IMPLEMENTED, UNVERIFIED on target | embedded template, special-token FIM validation, cancellation loops |
| Mistral.rs CPU compile | PASS | isolated hosted worker check, no editor image dependency change |
| Mistral process adapter | PASS, fixture tests | wire parsing and blocked-write cancellation, actual target still untested |
| Mistral FIM | NOT IMPLEMENTED | explicit unsupported error; chat only |
| Mistral cooperative cancellation | NOT IMPLEMENTED | hard cancellation kills/unloads worker instead |
| CUDA compilation, each backend | BLOCKED on toolkit check | owner build.sh paths supplied; not yet run with CUDA |
| Exact-model inference, each compatible backend | BLOCKED on owner execution | laptop not reachable; no generic Qwen compatibility claim |
| Actual GPU utilization/VRAM during inference | BLOCKED | logs and process-specific telemetry required, detection is insufficient |
| Benchmark collection | PASS, fixture tests | POSIX sh/Python, hash guard, same-model driver, comparison guard |
| Benchmarks, 3+ identical runs | BLOCKED | no target-machine runs recorded |
| TTFT/incremental worker streaming | NOT IMPLEMENTED | current worker returns one completion, metrics stay null |
| Configurable sampling profiles | PARTIAL | Candle FIM/chat temperature/top-k/top-p/seed implemented and tested; cross-provider parity and penalties/min-p still open |
| Download resume/cancellation/resource improvements | OPEN | curated checksums preserved; wider model management still under review |
| Production backend recommendation | BLOCKED | no valid comparative measurements, Candle retained |

No performance numbers are supplied because no target-model inference measurements
exist. CPU compilation is not an inference pass. CUDA compilation will not be a GPU
pass. Do not run memory-pressure tests without a separate bounded workload plan.

The current exact target does not fit entirely in 8 GB VRAM. Placement settings must
be tested for this file, context and runtime. Do not enable all layers on GPU as a
supposed safe default. Candle's qwen2 loader rejects qwen35moe, independently of CUDA.
