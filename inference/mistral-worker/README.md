# Isolated Mistral.rs worker candidate

The SDK's image dependency conflicts with the editor pin. This separate Cargo
workspace keeps the editor graph unchanged. The SDK is commit-pinned and its
lockfile is included. This is an implementation candidate, not accepted software.

Build CPU: `cargo build --release --manifest-path inference/mistral-worker/Cargo.toml`.
CUDA candidate: add `--features cuda` with the owner's local CUDA toolkit.
Set HF_HUB_OFFLINE=1 and LAPCE_AI_DEVICE=cpu (default) or cuda (explicit build
required). Model paths are canonical existing local files. Missing auxiliary
files fail offline, never trigger a Hub download. No requantization is requested.

Reads the existing Request JSON shape and returns Completion JSON. Currently
chat only, deterministic sampling, output1..512, context4096, one sequence.
Raw FIM returns an error. Hard process cancellation is the only supported
cancellation mechanism; cooperative cancellation is explicitly not claimed.
The editor provider adapter is still to be wired. Use bench/worker.py with the
worker executable for direct tests only after compilation passes.

No exact-model inference, GPU allocation or performance result has been recorded.
GPU selection is requested, not demonstrated. CPU/CUDA feature builds and actual
model tests remain acceptance gates. Native logs must stay separate from JSON.
Source: https://github.com/EricLBuehler/mistral.rs at
21da642fdd2210c30ca4028e1eabdd28e9c602fd .
