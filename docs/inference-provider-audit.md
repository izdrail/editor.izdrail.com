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

Optional Candle CUDA path: published0.9.1 APIs support device construction and
quantized_qwen2 loading/tensors on the selected device. Added candle-cuda feature,
CPU remains default, explicit cuda fails closed, explicit auto logs fallback.
Weights and decode inputs now use the same selected device. No partial offload
claim, no exact-target qwen35moe support claim, no GPU PASS. CPU cargo check and
27 crate tests passed locally,1 real-model test remains ignored. Hosted full
workspace and CUDA compilation/inference are still required.

Alternative backend implementation candidate: optional llama-cpp-2 pinned0.1.161
(MIT OR Apache-2.0), default features off, separate llama-cpp-cuda switch.
Local LAPCE_AI_BACKEND=llama-cpp selects its actual load/tokenize/prefill/greedy
decode loop through Provider. Explicit layer count and CPU expert placement
are opt-in; chat fails clearly until model template handling is implemented.
FIM requires real special tokens. Current worker wire shapes stay unchanged.
Native compile and inference acceptance are pending. Initial local native build
required libclang and headers; compilation exceeds one execution window, so
a dedicated standard hosted CPU feature check owns the complete build/test.
No model/backend/CUDA success claim is made from this candidate.
Sources inspected: https://crates.io/api/v1/crates/llama-cpp-2/0.1.161/download
and https://github.com/utilityai/llama-cpp-rs .

llama.cpp chat follow-through: use the exact model's embedded template through
the binding's chat_template/apply_chat_template API, validate roles, escape
reserved message markers as in the existing engine, tokenize rendered template,
and preserve raw chat output. Unsupported templates return an error. Registry
advertises the compiled experimental backend with explicit pending-acceptance
limits, not unproven architecture/CUDA claims.

Hosted optional llama.cpp CPU build and crate tests passed for a84257f and
9126e34, including embedded chat-template code. This is compilation, not
exact-model inference. The9126 full-workspace run separately hit ETXTBSY while
launching a temporary shell test fixture. Fixture publication now uses unique
paths and a fully closed/synced staging file before atomic rename. No timeout
threshold or production behavior changed.

Mistral.rs integration finding: pinned upstream commit
21da642fdd2210c30ca4028e1eabdd28e9c602fd uses image^0.25.10 while lapce-app
pins image=0.25.1. Adding the optional SDK to this workspace fails dependency
resolution even with its feature disabled. Do not silently upgrade the editor's
image stack to hide this conflict. Evaluate a separately built local worker
adapter with its own dependency graph, preserving the current editor protocol.
SDK implementation remains open. Source inspected:
https://github.com/EricLBuehler/mistral.rs , mistralrs/Cargo.toml and the local
GGUF builder example. Its APIs offer local GGUF loading and deterministic bounded
chat requests, but local-only auxiliary-file and cancellation behavior still
need source verification before execution.

Isolated Mistral.rs implementation candidate added under inference/mistral-worker
with an independent714-package lockfile, resolving the image conflict without
editor dependency changes. Local GGUF path, HF_HUB_OFFLINE=1 mandatory, CPU
default/explicit CUDA feature, deterministic bounded chat, Completion JSON
metrics from SDK usage. Raw FIM and cooperative cancellation fail explicitly;
hard cancellation and editor adapter integration remain open. Added hosted
isolated CPU compile gate. Resolution passed, compilation/inference pending.

MistralRsProvider process adapter wired through the existing Provider trait and
worker backend selector. Explicit executable path, offline child environment,
8MiB request/response bounds,300s response deadline,25ms cancellation polling,
kill/wait cleanup and cold restart after cancellation. Fixture regression checks
completion JSON and cancellation cleanup;28 default crate tests passed locally.
This verifies the adapter, not SDK compilation or actual model inference.
