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
