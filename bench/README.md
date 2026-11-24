# Owner-run inference measurements

This package collects measurements. It does not install providers, download models,
select a faster quantization, clear the machine's page cache, or prove CUDA execution.
The alternative providers and exact-model inference are not yet accepted.

Requirements: Linux, POSIX sh, Python 3, sha256sum. GPU telemetry requires
nvidia-smi. Build commands and feature combinations are still under implementation;
do not interpret this collector as a finished CUDA build package.

Set MODEL to the exact target GGUF and EXPECT_SHA256 to its independently verified
SHA256. Set CONFIG to a JSON string describing prompt, context, output count, sampling,
seed and offload parameters. Record BUILD_CMD, CARGO_FEATURES and PROVIDER_VERSION.
No path defaults to a different model. A missing hash or mismatch stops the run.

    MODEL=/path/to/exact.gguf EXPECT_SHA256=verified_hash CONFIG='{"context":2048}' \
      sh bench/run.sh candle short 1 -- /path/to/worker-benchmark-command

The command is executed directly, never through eval. Its final stdout line may be a
JSON object containing measured load_ms, ttft_ms, prompt_tps, gen_tps, tokens and
cancel_ms. Missing metrics remain missing. The worker's existing JSON protocol is
not automatically a complete benchmark driver. A scenario is a label, not proof the
command performed that scenario. Cold-cache verification is always false here.

Use three or more runs for each identical workload. Keep raw stdout, stderr, samples
and JSON. Existing run paths are not overwritten. Run nothing else on the GPU.
RSS and CPU cover only the launched process, not children. GPU readings cover the
whole device and cannot establish CUDA PASS. Actual process allocation, inference
logs and correct output must independently corroborate GPU use. Multiple GPUs leave
samples blank rather than silently selecting one. Do not run pressure tests on the
laptop without explicit limits in the benchmark command.

No results have been fabricated or prefilled. Target-machine execution remains
blocked until this package and provider build steps can be run on the owner's laptop.

## Metadata inspection

`python3 bench/inspect_gguf.py /path/to/model.gguf` parses metadata and tensor
type counts without loading weights. `target-model-header.json` records the
24 November2026 remote range inspection. Architecture is qwen35moe, not qwen2.
The server's full-file LFS checksum is provenance, not a locally computed checksum
of weights. The collector still requires verifying the owner's complete file.

Run `python3 bench/test_bench.py` for non-inference fixtures. Aggregate a single
comparison environment with `python3 bench/collect.py /path/to/results`. Mixed
model/hardware/configuration runs are rejected. Failed runs do not contribute
performance numbers; fewer than three successful runs are marked. CUDA status
remains unverified regardless of device-wide telemetry.

## Candle device build path (runtime validation pending)

CPU remains the default. Build the optional CUDA path from an installed CUDA
toolkit with `cargo build --release -p lapce-ai --features candle-cuda`.
Set `LAPCE_AI_DEVICE=cuda` to require CUDA; unavailable CUDA fails, not silently
CPU. `LAPCE_AI_DEVICE=auto` permits an explicitly logged CPU fallback.
`LAPCE_AI_DEVICE=cpu` forces the unchanged CPU path. Device choice logs go to
stderr; worker JSON stays on stdout. This is full-device Candle placement,
not partial MoE offload. It still rejects the qwen35moe target architecture.
CUDA compilation and actual GPU inference are NOT yet verified.

## Optional llama.cpp candidate (build/runtime verification pending)

Build CPU: `cargo build --release -p lapce-ai --features llama-cpp`.
Build CUDA with local toolkit: `cargo build --release -p lapce-ai --features llama-cpp-cuda`.
Set LAPCE_AI_BACKEND=llama-cpp, LAPCE_AI_GPU_LAYERS to an explicit layer
count (default0), and LAPCE_AI_CPU_EXPERTS=1 to request CPU-resident MoE experts.
These are placement requests, not proof that the target model fits or executes
on GPU. Inspect native logs, process VRAM and actual inference independently.
FIM requires model special tokens; chat currently returns an unsupported error.
The exact target may be chat-only; do not interpret that error as successful
benchmark execution. Full exact-model/chat support remains an implementation item.
