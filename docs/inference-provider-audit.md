# Inference implementation audit, 24 November 2026

Current lapce-ai uses Candle0.9.1, CPU-only quantized_qwen2, with explicit Ollama remote requests. Keep both and the existing serialized Request/Completion fields. Downloads remain explicit. The exact qwen35moe target is not supported by this loader; no runtime success claim is made.

First bounded fix: native Ollama done chunks already expose token counts and nanosecond timings, but both FIM and chat discarded them. Parse and return these through existing metrics fields. Use measured wall elapsed only when total_duration is missing. Preserve raw chat text and FIM cleanup. Surface native server error frames. Focused regression plus full hosted workspace tests required.

Next: shared bounded stream parsing and HTTP reuse; configurable generation profiles and discovery/capabilities; validate Candle CUDA APIs and model architecture; optional alternative backends after version/API verification; CPU and CUDA build checks; POSIX sh benchmark package for the owner laptop. No architecture rewrite is assumed necessary.

GPU runtime acceptance is blocked on the owner-run RTX3080 Laptop8GB/48GBRAM. Compilation or detection is not GPU inference evidence. Partial offload needs backend-specific verification. Keep the exact target model/quantization for comparisons and never invent measurements.

Stage2: shared stream parser bounds each line before unbounded allocation (1MiB) and cumulative response(8MiB), preserving both wire shapes and Unicode. Stage3: one reusable ureq Agent per provider enables connection pooling; explicit connect/read/write timeout bounds, no redirects,8MiB serialized request limit in both lanes. Default generation parameters and request schema stay unchanged. These are reliability/architecture changes, not measured latency claims. Network cancellation during a blocked read still waits for its timeout; further work required.

Worker follow-through: preserve the Ollama provider between identical sequential remote requests, not only inside one completion. Replace it on endpoint, model or timeout changes. Local requests still never trigger network execution. Added configuration-switch regression coverage. No measured speed claim.

Build discovery: `lapce-ai --ai-capabilities` emits machine-readable implemented backend capabilities without loading files or making network requests. Reports current CPU-only Candle/qwen2, explicit Ollama execution, existing sampling and cancellation limits. Does not advertise alternative backends or CUDA until implemented and checked. Request/Completion shapes unchanged.

Benchmark collection: `bench/run.sh` is POSIX sh, dispatching to a standard-library Python collector. Requires an exact model hash, preserves raw output, records environment and telemetry, refuses overwrite. Fixture tests exercise collection and hash refusal only. It is not yet the complete provider build/benchmark package.
