# Inference implementation audit, 24 November 2026

Current lapce-ai uses Candle0.9.1, CPU-only quantized_qwen2, with explicit Ollama remote requests. Keep both and the existing serialized Request/Completion fields. Downloads remain explicit. The exact qwen35moe target is not supported by this loader; no runtime success claim is made.

First bounded fix: native Ollama done chunks already expose token counts and nanosecond timings, but both FIM and chat discarded them. Parse and return these through existing metrics fields. Use measured wall elapsed only when total_duration is missing. Preserve raw chat text and FIM cleanup. Surface native server error frames. Focused regression plus full hosted workspace tests required.

Next: shared bounded stream parsing and HTTP reuse; configurable generation profiles and discovery/capabilities; validate Candle CUDA APIs and model architecture; optional alternative backends after version/API verification; CPU and CUDA build checks; POSIX sh benchmark package for the owner laptop. No architecture rewrite is assumed necessary.

GPU runtime acceptance is blocked on the owner-run RTX3080 Laptop8GB/48GBRAM. Compilation or detection is not GPU inference evidence. Partial offload needs backend-specific verification. Keep the exact target model/quantization for comparisons and never invent measurements.

Stage2: shared stream parser bounds each line before unbounded allocation (1MiB) and cumulative response(8MiB), preserving both wire shapes and Unicode. Stage3: one reusable ureq Agent per provider enables connection pooling; explicit connect/read/write timeout bounds, no redirects,8MiB serialized request limit in both lanes. Default generation parameters and request schema stay unchanged. These are reliability/architecture changes, not measured latency claims. Network cancellation during a blocked read still waits for its timeout; further work required.

Worker follow-through: preserve the Ollama provider between identical sequential remote requests, not only inside one completion. Replace it on endpoint, model or timeout changes. Local requests still never trigger network execution. Added configuration-switch regression coverage. No measured speed claim.

Build discovery: `lapce-ai --ai-capabilities` emits machine-readable implemented backend capabilities without loading files or making network requests. Reports current CPU-only Candle/qwen2, explicit Ollama execution, existing sampling and cancellation limits. Does not advertise alternative backends or CUDA until implemented and checked. Request/Completion shapes unchanged.

Benchmark collection: `bench/run.sh` is POSIX sh, dispatching to a standard-library Python collector. Requires an exact model hash, preserves raw output, records environment and telemetry, refuses overwrite. Fixture tests exercise collection and hash refusal only. It is not yet the complete provider build/benchmark package.

Exact-target pre-integration check completed from a fresh bounded range request:
https://huggingface.co/unsloth/Qwen3.6-35B-A3B-GGUF/resolve/main/Qwen3.6-35B-A3B-UD-Q4_K_XL.gguf .
Parsed full metadata and733 tensor descriptors, header length10990035 bytes.
Architecture qwen35moe,40 blocks,256 experts/8 active, attention interval4;
tensor type IDs0/8/12/13/14, server file size22360456160 bytes. Full payload
not downloaded or hashed. Machine-readable provenance in bench/target-model-header.json.
This verifies the format/architecture, not backend inference compatibility.

Candle compatibility guard: before loading weights, read actual GGUF context
length and embedding vocabulary size. Check external tokenizer IDs against the
embedding range and, when present, the GGUF token-ID mapping. Completion rejects
prompt-plus-output budgets over the model's context before forward(), and checks
cancellation before/after tokenization. Synthetic vocabulary regression tests
cover mismatched IDs, swapped mappings and missing entries. Real-model regression
remains required; this does not establish support for qwen35moe.

Candle0.9.1 source API verified from its published crate and quantized_qwen2
loader: https://crates.io/api/v1/crates/candle-core/0.9.1/download and
https://crates.io/api/v1/crates/candle-transformers/0.9.1/download .
The loader already resets KV cache at position0 and then decodes only the newest
token. No fabricated KV-cache speedup is claimed.

Explicit discovery: `lapce-ai --ai-ollama-models ENDPOINT [MODEL]` reads
/api/tags or /api/show, never /api/pull. The endpoint is mandatory. Responses
are bounded1MiB, timeouts bounded, redirects disabled. Model capabilities stay
server-reported, with no inference of FIM/tool compatibility. Loopback tests
verify exact paths/methods and show-body model selection.
API references: https://docs.ollama.com/api/tags and https://docs.ollama.com/api/show .
