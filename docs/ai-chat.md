# AI Chat (v1)

Open the right-hand **AI Chat** panel from its lightbulb sidebar button or the
**Toggle AI Chat** command. Type a question and press Enter or Send. New chat
clears the conversation. History is per window tab and kept in memory only.

## Izdrail Ollama first

Settings > AI Chat defaults to `backend = "ollama"` against
`https://ai.izdrail.com`, Stefan's key-less server: chat, Ask and Plan work out
of the box with no keys and no hosted fallback (verified on `/api/tags` and
`/v1/models` 2026-10-17). Endpoint and model stay editable.

## Local lane (opt-in)

Set `backend = "local"` to use Candle on CPU in an isolated, reusable worker
process, not Ollama or a hosted API. Choose a Chat model in the curated model
list, Download selected model, then Use this model. Downloads are explicit;
they never upload editor content. The curated default is
Qwen2.5-Coder-1.5B-Instruct Q4_K_M (986 MB). Local replies arrive as complete
turns. Chat uses Qwen ChatML, not the FIM autocomplete template. The instruct
model has a separate manifest and cannot replace the autocomplete base model.

The picker includes FIM 1.5B and 3B, and Chat Instruct 0.5B, 1.5B and 3B. Each
artifact is pinned to an immutable Hugging Face revision, byte size and SHA-256
in `lapce-ai/src/catalog.rs`. All use the official pinned Qwen tokenizer, also
size/hash verified. QuantFactory and bartowski provide the GGUF conversions;
Qwen models are Apache-2.0. Larger models run more slowly on CPU. Free RAM
figures shown in the picker are conservative planning estimates, not measured
benchmarks or guarantees. Allow additional RAM for the editor and OS. The
research bundle's RAM measurements were not available during this change.

Use this model switches only its matching lane (FIM or Chat) after a successful
download; no path editing is needed. Existing manual FIM paths remain supported
and override the picker. Clear those overrides to use a curated FIM choice.

## Your LLM server

`backend = "ollama"` is the default: any OpenAI-compatible `/v1` server.
Endpoint defaults to `https://llm.izdrail.com`, Stefan's gateway (~197
models, `auto` routing - the default model is `auto`). The gateway rejects
key-less calls: set `LAPCE_LLM_API_KEY` in the editor's environment and it
rides every chat, agent, model-list and capability request as
`Authorization: Bearer`. His key-less Ollama at `https://ai.izdrail.com`
stays selectable in the endpoint dropdown. (freellm.izdrail.com is not his
address and answers 503 on every path - removed.)
Endpoint and model are editable - point them at any OpenAI-compatible server.
No OpenAI key, OpenAI service or hidden hosted fallback is used. Errors stay
in the panel, and a 401/403 names the LAPCE_LLM_API_KEY fix. Verified against
the live gateway 2026-10-19 (401 without a key; Ollama emulation disabled).

Responses stream when the server returns SSE. Servers that reject streaming
with 400/405/422/501 are retried once in full-response mode. A broken partial
stream is not retried automatically, and its text stays visible.

## Context and limits

The panel shows the file/selection name, byte count and any truncation before
Send and records it with each user turn. A non-empty selection is used instead
of the file; otherwise the active file/scratch buffer is used, including unsaved
changes. Only the first 12,000 UTF-8-safe bytes are included, never the whole
repository. No automatic requests occur while typing or switching files.
Local mode keeps context on the device. Ollama mode sends it to the configured
endpoint when you press Send. Do not send secrets to a server you do not trust.

The last four successful turns are retained in request context. Local requests
have an 8192-token prompt/output budget, with at most 512 output tokens. An
oversized request gives an error asking for a new chat or smaller selection.
Source context is data, not instructions; literal ChatML markers in content are
neutralised so they cannot forge message roles. Replies do not run tools or
change files. The v1 UI is single-line input, with code-block/Markdown display.

Future, not implemented: @-mentions, agent mode, tool calls, multi-file context,
file edits, persistent history, stop button and local token streaming.

## Verification

Run `cargo check -p lapce-app` and `cargo test -p lapce-app --lib chat::tests`.
For template tests, set `AI_TOKENIZER` to the official tokenizer and run
`cargo test -p lapce-ai --lib`. Real chat quality and CPU latency need a model
and a host benchmark; compile success is not a quality benchmark.

## Slash commands (item 139)

The composer accepts four slash commands, an honest subset of
Antigravity's catalog (verified against antigravity.google/docs/slash-commands
2026-10-17):

- `/plan <message>` - run the message in Plan mode; the reply also lands
  as a reviewable `.lapce/plans/` artifact.
- `/ask <message>` - run the message in Ask mode.
- `/btw <message>` - one-off side question: answered like chat, but not
  added to the conversation history and not persisted. The turn is
  labeled "Side question" in the panel.
- `/learn <note>` - append a project knowledge note to
  `.lapce/knowledge.md` (item 159). Later chat sends in this project
  fold the newest notes (tail, capped at 4 KiB) into the prompt as
  read-only `<project_knowledge>` context. Notes are user-curated: the
  model never writes the file, and Ask/Plan keep their own prompt
  builders and do not read it. Edit the file directly to revise or
  prune notes.

Commands override the Chat/Ask/Plan/ACP mode buttons for that one send.
`/learn` stores its note and never reaches the model. Unknown commands
and bare commands are rejected with a hint instead of silently chatting.
`/goal` is deliberately absent: it implies autonomous execution, which
is the parked Act mode. In ACP mode `/btw` behaves like
a normal send (ACP turns never join chat history anyway).

## Any OpenAI-compatible server (item 140)

The ollama backend speaks the plain OpenAI `/v1/chat/completions` wire
format (SSE streaming with a full-response retry on 400/405/422/501, no
auth), so it also works against vLLM, LM Studio, llama.cpp's server and
similar self-hosted endpoints: set `backend = "ollama"`, point Endpoint
at the server (root, `/v1`, or a full `/v1/chat/completions` URL are all
accepted) and set Model to a name the server lists. Error text refers to
"the chat server", not Ollama, for this reason. Servers requiring an API
key are not supported yet; Ask/Plan tool calls stay Ollama-specific.

The Endpoint field is a dropdown with the two key-less endpoints Stefan
runs: `https://ai.izdrail.com` (his Ollama, the default) and
`https://freellm.izdrail.com`. A custom URL typed in settings.toml stays
selectable. The same dropdown exists under AI Autocomplete.
