# Inference implementation audit, 24 November 2026

Current lapce-ai uses Candle0.9.1, CPU-only quantized_qwen2, with explicit Ollama remote requests. Keep both and the existing serialized Request/Completion fields. Downloads remain explicit. The exact qwen35moe target is not supported by this loader; no runtime success claim is made.

First bounded fix: native Ollama done chunks already expose token counts and nanosecond timings, but both FIM and chat discarded them. Parse and return these through existing metrics fields. Use measured wall elapsed only when total_duration is missing. Preserve raw chat text and FIM cleanup. Surface native server error frames. Focused regression plus full hosted workspace tests required.

Next: shared bounded stream parsing and HTTP reuse; configurable generation profiles and discovery/capabilities; validate Candle CUDA APIs and model architecture; optional alternative backends after version/API verification; CPU and CUDA build checks; POSIX sh benchmark package for the owner laptop. No architecture rewrite is assumed necessary.

GPU runtime acceptance is blocked on the owner-run RTX3080 Laptop8GB/48GBRAM. Compilation or detection is not GPU inference evidence. Partial offload needs backend-specific verification. Keep the exact target model/quantization for comparisons and never invent measurements.
