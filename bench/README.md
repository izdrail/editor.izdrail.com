# Owner-run inference measurements

This package collects measurements. It does not install providers, download models,
select a faster quantization, clear the machine's page cache, or prove CUDA execution.
The alternative providers and exact-model inference are not yet accepted.

Requirements: Linux, POSIX sh, Python 3, sha256sum. GPU telemetry requires
nvidia-smi. Explicit build commands are supplied below. CUDA compilation and inference
remain unverified; this is not an accepted GPU package.

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
RSS and CPU sum the launched process and live descendants, with the limitations
listed below. GPU readings cover the whole device and cannot establish CUDA PASS. Actual process allocation, inference
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

## Optional llama.cpp candidate (CPU build passed, target runtime pending)

Build CPU: `cargo build --release -p lapce-ai --features llama-cpp`.
Build CUDA with local toolkit: `cargo build --release -p lapce-ai --features llama-cpp-cuda`.
Set LAPCE_AI_BACKEND=llama-cpp, LAPCE_AI_GPU_LAYERS to an explicit layer
count (default0), and LAPCE_AI_CPU_EXPERTS=1 to request CPU-resident MoE experts.
These are placement requests, not proof that the target model fits or executes
on GPU. Inspect native logs, process VRAM and actual inference independently.
FIM requires model special tokens; chat uses the model's embedded template and
returns a clear error if it cannot be applied. Exact-model support remains
unverified until inference succeeds.

## Execute a local worker request

Create request.json with the existing Request fields (model path, tokenizer path,
prefix, suffix, token budgets, max_tokens, request_id, optional messages, remote:null).
For llama.cpp chat, set messages to system/user/assistant objects; tokenizer path is
not used because its tokenizer comes from the GGUF. Keep the exact approved model.
The request file is private input and is not bundled into uploaded results by default.

    MODEL=/path/to/exact.gguf EXPECT_SHA256=verified_hash CONFIG='same-workload' \
      LAPCE_AI_BACKEND=llama-cpp sh bench/run.sh llamacpp short 1 -- \
      python3 bench/worker.py --worker target/bench-builds/llamacpp-cpu/release/lapce-ai --request request.json

Use --repeat 3 for sequential warm requests in one loaded process. Separate process
invocations give load-inclusive runs, not verified cold-page-cache measurements.
The driver enforces a response deadline and kills the process group on failure.
It preserves raw completions on stderr and emits measured final-decode tokens/s
only when decode_ms is nonzero. TTFT, streaming latency and prompt throughput
remain null because the current worker protocol does not expose those observations.
Review output quality separately. A completed command alone is not model acceptance.

Ignored real-model regression (owner-run only, no download):

    AI_MODEL=/path/to/exact.gguf cargo test -p lapce-ai --features llama-cpp \
      real_model_chat_and_cancellation -- --ignored --nocapture

No results from that test have been recorded here yet.

The fixture suite also checks three repeated worker requests and refusal when
the request model is not the same file as MODEL. This prevents collecting a
verified hash for one file while benchmarking a different file.

## Explicit owner build helper

`sh bench/build.sh candle cpu`, `sh bench/build.sh llamacpp cuda`, or
`sh bench/build.sh mistralrs cuda` uses installed tools and pinned lockfiles.
It can fetch source dependencies through Cargo, but never downloads model weights
or installs a toolkit. Save stdout/stderr as build provenance. GPU prerequisites
are checked only for requested CUDA builds. No model execution happens during build.
Each backend/device gets a distinct build directory. Record the printed binary
path and feature settings, and use that exact path in the driver. See ACCEPTANCE.md
for the current unverified and unimplemented areas before running comparisons.

Candle optional sampling profiles: LAPCE_AI_CANDLE_FIM_SAMPLING and
LAPCE_AI_CANDLE_CHAT_SAMPLING accept JSON, for example
`{"temperature":0.7,"top_k":40,"top_p":0.95,"seed":42}`. They apply only
to Candle. Missing profiles keep greedy defaults. Unknown keys and invalid
values fail, and greedy temperature0 rejects top-k/top-p rather than ignoring
them. Record profile JSON in CONFIG for comparisons. These settings do not
establish sampling parity with another backend.

The local worker driver bounds requests/responses to8MiB. Its deadline covers
blocked pipe writes and incomplete response lines, not only the first output
byte. A deadline or malformed output fails the run and terminates the process
group. Fixture tests cover non-reading and partial-line workers.

Candle profiles also accept repetition_penalty (default1, range0.01..100),
frequency_penalty and presence_penalty (default0, range-2..2), and up to256
stop_token_ids validated against the tokenizer vocabulary. Penalties apply to
generated output only, not the prompt. Repetition divides nonnegative logits
and multiplies negative logits once per distinct token. Presence subtracts once
per seen token; frequency subtracts its value times the output count. Additional
stops supplement built-in model/FIM stops, never remove them. Min-p remains
unsupported and is rejected. These settings do not change other backends.

Candle LAPCE_AI_DEVICE accepts cpu (unchanged default), cuda (device0),
cuda:N (explicit nonnegative index below256), or auto. Explicit cuda:N errors
if unavailable, never silently falls back. Auto currently only probes device0
and falls back on initialization failure. VRAM/context-aware auto placement is
not implemented, so use cpu or a verified explicit device for now. Neither
selection nor initialization establishes that an inference ran on the GPU.

Build isolation: build.sh now writes to target/bench-builds/BACKEND-DEVICE,
not a shared release directory. Set BENCH_BUILD_ROOT to move that parent.
CARGO_TARGET_DIR is set by the helper deliberately. Use the exact Worker path
printed by each invocation; older target/release examples are not helper output.
This prevents a later CPU build replacing the CUDA binary before comparison.
For Mistral, separately build the editor bridge and set LAPCE_AI_MISTRAL_WORKER
to the isolated Mistral worker path, with LAPCE_AI_BACKEND=mistral-rs.

Resource telemetry now sums the command and its live descendants, including
model workers launched by the Python driver and isolated Mistral bridge.
process_count is recorded per sample. Summed RSS can double count shared pages,
and ps CPU percentages are lifetime averages, not per-interval utilization.
Short-lived or already-reparented children may be missed. Device-wide GPU
samples still do not establish process-specific CUDA inference. Earlier results
labelled command-process-only must not be compared as equivalent memory scope.

## Before recording a comparison

Read ACCEPTANCE.md first. A repeat count is not three independent cold runs.
worker.py reports the final request at the top level and keeps every repeated
request in results; repeated load_ms values describe the retained model load,
not a fresh load for each request. TTFT and cancellation latency remain unknown.

The benchmark tools do not choose context, GPU layers or CPU expert placement.
Supply bounded settings explicitly and keep MODEL, quantization, prompts, output
budgets and generation settings identical. Record the full command and profile in
CONFIG. Do not compare an unsupported Candle load against a successful runtime
as though that were a speed comparison. The exact qwen35moe target cannot be a
Candle CPU/CUDA benchmark until a compatible loader exists. A smaller compatible
Candle model would be a separate experiment, never a replacement target result.
