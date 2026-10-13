# Local AI autocomplete (experimental)

Settings -> AI Autocomplete enables manual local ghost text. Default is off.
Download Qwen2.5-Coder 1.5B BASE Q4_K_M explicitly using the download action in Settings. It fetches approximately 986 MB weights plus 7 MB tokenizer from Hugging Face. Publisher: QuantFactory conversion of Qwen's base model. Licence: Apache-2.0. Source code is never sent to the Hub. No automatic download or update happens during completion.

Source: https://huggingface.co/QuantFactory/Qwen2.5-Coder-1.5B-GGUF
Upstream: https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B
Licence: https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B/blob/main/LICENSE
Hub terms: https://huggingface.co/terms-of-service

The downloader uses pinned immutable revisions, resumable Hub partial files, locks and progress callbacks, then independently verifies size and SHA-256 before writing installed.json. Cancel kills the download process; click Download again to resume. Files live under the editor's local data directory in ai-models/hub; portable mode follows the existing portable data directory. A bad cached hash causes an error, never Ready. Remove the invalid editor-owned blob to retry; automatic repair and uninstall UI are not implemented.

Settings:

```toml
[ai-autocomplete]
enabled = false
model-path = "" # downloaded model by default
tokenizer-path = "" # matching downloaded tokenizer by default
prefix-tokens = 512 # 1..2048
suffix-tokens = 128 # 0..1024
max-tokens = 32 # 1..128
```

In Insert mode, Alt+Backslash invokes completion using the existing inline command. Existing inline accept/cancel bindings remain. Automatic invocation is intentionally disabled. Inference is CPU-only in a child process and never on the UI/dispatcher thread. The provider owns FIM prompting, token budgets, greedy sampling and marker stops over Candle's quantized Qwen2 architecture. It does not implement new kernels or duplicate Candle's transformer layers. Model files are lazy-loaded and reused while the worker remains alive. Edits/cancellation kill the worker; the next request reloads it. Stale responses are rejected against generation, document, revision and cursor. No memory-pressure monitor or idle unload is implemented.

Local model/tokenizer overrides require a Qwen2 GGUF with matching Qwen FIM tokenizer. Other architectures are rejected. Scratch-buffer and remote-workspace model placement are not yet supported consistently: download occurs on the UI host, while inference follows the proxy host. Use a local workspace for this first version. An HTTP provider is not implemented; the Provider trait is the extension seam.

Verification commands:

```sh
cargo check --workspace
AI_TOKENIZER=/path/tokenizer.json cargo test -p lapce-ai
AI_MODEL=/path/model.gguf AI_TOKENIZER=/path/tokenizer.json cargo test -p lapce-ai --release real_fim -- --ignored --nocapture
cargo build --release --bin lapce --bin lapce-proxy --bin lapce-ai
```

The real FIM test has small Rust/PHP fixtures. It checks non-empty code-shaped output, not broad completion quality. This is experimental until real UI rendering, accept/undo, cancellation/download recovery and host-memory tests pass. 1.5B loading was SIGKILLed in a 1.9 GiB/no-swap workspace; downloaded weight size is not its runtime RAM budget.
