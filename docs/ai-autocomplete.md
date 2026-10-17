# AI autocomplete (ghost text)

Settings -> AI Autocomplete controls inline ghost text at the cursor. Default
is on, served by the key-less Ollama at https://ai.izdrail.com with the
verified qwen2.5-coder:1.5b-instruct-q4_K_M model. Two backends exist:

- **ollama (default, network).** Each suggestion sends the bounded code
  around the cursor to the configured endpoint's native /api/generate with a
  raw Qwen FIM prompt. Fast enough for automatic suggestions after a typing
  pause.
- **local (opt-in, on device).** CPU Candle inference over a Qwen2.5-Coder
  1.5B GGUF you download explicitly. Nothing is sent anywhere; each CPU
  suggestion takes seconds.

Any backend string other than `ollama` selects the local lane, so a typo can
never silently enable network sending.

## The no-network guarantee, plainly

- `backend = "local"`: completion makes **zero network requests**. The only
  network use on this lane is the explicit Download button in Settings, which
  fetches the model weights from Hugging Face (details below) and is never
  triggered by typing or completion.
- `backend = "ollama"`: on each manual or automatic trigger, the text up to
  ~16 KB before and ~8 KB after the cursor plus the model name is sent over
  TLS to the endpoint shown in Settings (default https://ai.izdrail.com). No
  API key exists or is sent. Nothing else leaves the device: no file paths,
  no workspace scans, no telemetry.
- `enabled = false`: no completion requests of any kind, on either backend.

## Settings

```toml
[ai-autocomplete]
enabled = true
backend = "ollama" # or "local" for fully on-device
endpoint = "https://ai.izdrail.com" # ollama lane only
remote-model = "qwen2.5-coder:1.5b-instruct-q4_K_M" # ollama lane only
timeout-ms = 15000 # 1000..120000, ollama lane only
local-model = "qwen-fim-1.5b" # local lane: curated download
model-path = "" # local lane: explicit Qwen2 GGUF overrides the download
tokenizer-path = "" # local lane: matching tokenizer.json
prefix-tokens = 512 # 1..2048
suffix-tokens = 128 # 0..1024
max-tokens = 32 # 1..128
auto-trigger = true # automatic ghost text after a typing pause
auto-trigger-debounce-ms = 400 # 50..2000
```

In Insert mode, Alt+Backslash requests a suggestion immediately; with
auto-trigger on, one appears after each typing pause. Existing inline
accept/cancel bindings remain. Automatic requests never fire while the
completion popup is open, outside Insert mode, or on remote/scratch buffers;
undo and other non-typing edits only cancel.

## How a request flows

The editor slices the bounded prefix/suffix on char boundaries and hands a
JSON request to the proxy dispatcher, which queues it on a reusable worker
process - never on the UI/dispatcher thread. At most one request is in
flight and one pending; a newer request supersedes the pending one (latest
wins), so automatic triggers cannot pile up. Every keystroke cancels the
in-flight request cooperatively. Stale responses are rejected against
generation, document, revision and cursor.

On the ollama lane the worker POSTs with `raw: true` (no chat template),
streams chunks, and checks the cancellation flag between chunks, so a soft
cancel aborts mid-generation. Server-side stop markers
(`<|fim_pad|>`, `<|endoftext|>`, the FIM markers, repo/file separators) are
re-applied client-side, and any tail duplicating the buffer suffix is
clipped. A network failure reports an RPC error for manual triggers and is
silent for automatic ones; the worker restarts on the next request with
nothing cached.

On the local lane the worker loads the GGUF once and keeps it warm; soft
cancellation stops between tokens without a reload. Cancellation during the
prompt prefill (one uninterruptible forward pass) completes the pass first,
so a cancel can take about one prefill plus one token. Shutdown still kills
the worker. No memory-pressure monitor or idle unload is implemented.

## Local model download (local lane only)

Download Qwen2.5-Coder 1.5B BASE Q4_K_M explicitly using the download action
in Settings. It fetches approximately 986 MB weights plus 7 MB tokenizer
from Hugging Face. Publisher: QuantFactory conversion of Qwen's base model.
Licence: Apache-2.0. Source code is never sent to the Hub. No automatic
download or update happens during completion.

Source: https://huggingface.co/QuantFactory/Qwen2.5-Coder-1.5B-GGUF
Upstream: https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B
Licence: https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B/blob/main/LICENSE
Hub terms: https://huggingface.co/terms-of-service

The downloader uses pinned immutable revisions, resumable Hub partial files,
locks and progress callbacks, then independently verifies size and SHA-256
before writing installed.json. Cancel kills the download process; click
Download again to resume. Files live under the editor's local data directory
in ai-models/hub; portable mode follows the existing portable data
directory. A bad cached hash causes an error, never Ready. Remove the
invalid editor-owned blob to retry; automatic repair and uninstall UI are
not implemented.

Local model/tokenizer overrides require a Qwen2 GGUF with matching Qwen FIM
tokenizer. Other architectures are rejected. Scratch-buffer and
remote-workspace model placement are not yet supported consistently:
download occurs on the UI host, while inference follows the proxy host. Use
a local workspace for this first version. On a remote workspace the ollama
lane runs on the proxy host.

## Verification commands

```sh
cargo check --workspace
cargo test -p lapce-ai
AI_TOKENIZER=/path/tokenizer.json cargo test -p lapce-ai
AI_MODEL=/path/model.gguf AI_TOKENIZER=/path/tokenizer.json cargo test -p lapce-ai --release real_fim -- --ignored --nocapture
cargo build --release --bin lapce --bin lapce-proxy --bin lapce-ai
```

The unit tests make no network calls; endpoint normalization, FIM prompt
shape, marker cutting, suffix clipping and cancellation wiring are covered
with non-ASCII fixtures. The real FIM test has small Rust/PHP fixtures. It
checks non-empty code-shaped output, not broad completion quality. 1.5B
loading was SIGKILLed in a 1.9 GiB/no-swap workspace; downloaded weight size
is not its runtime RAM budget.
