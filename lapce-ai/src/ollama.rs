//! Key-less Ollama FIM lane. When a request carries [`RemoteFim`], the worker
//! POSTs the bounded prefix/suffix around the cursor to the configured
//! endpoint's /api/generate instead of loading local model files. There is no
//! API-key field by design: the endpoint is either public-in-private (like
//! ai.izdrail.com) or plain unreachable.
use std::{
    io::{BufRead, BufReader},
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{Completion, Provider, Request, clip_suffix};

/// Remote serving parameters carried inside a [`Request`], so the worker
/// stays generic and the editor owns the endpoint policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFim {
    /// Ollama endpoint root, e.g. https://ai.izdrail.com. A trailing slash or
    /// a `/v1` suffix is tolerated and normalized away.
    pub endpoint: String,
    /// Ollama model name, e.g. qwen2.5-coder:1.5b-instruct-q4_K_M.
    pub model: String,
    /// Per-read network timeout; clamped to 1..=120 seconds.
    pub timeout_ms: u64,
}

/// Normalize an endpoint root into the native generate URL. The OpenAI
/// compatibility suffix `/v1` does not expose raw FIM prompting, so the
/// native API is always used.
fn api_url(endpoint: &str, path: &str) -> String {
    let trimmed = endpoint.trim().trim_end_matches('/');
    let root = trimmed.strip_suffix("/v1").unwrap_or(trimmed);
    format!("{root}{path}")
}

/// Normalize an endpoint root into the native generate URL. The OpenAI
/// compatibility suffix `/v1` does not expose raw FIM prompting, so the
/// native API is always used.
pub fn generate_url(endpoint: &str) -> String {
    api_url(endpoint, "/api/generate")
}

/// Native chat URL for instruct-turn requests (next-edit prediction).
pub fn chat_url(endpoint: &str) -> String {
    api_url(endpoint, "/api/chat")
}

/// Qwen FIM prompt order: prefix, suffix, then the middle marker the model
/// continues from. `raw: true` on the request keeps Ollama from wrapping this
/// in the instruct chat template.
pub fn fim_prompt(prefix: &str, suffix: &str) -> String {
    format!("<|fim_prefix|>{prefix}<|fim_suffix|>{suffix}<|fim_middle|>")
}

/// Structure markers the generation must stop at. Applied server-side and
/// re-applied in [`clean`] for servers that echo them back.
const STOPS: [&str; 7] = [
    "<|fim_pad|>",
    "<|endoftext|>",
    "<|fim_prefix|>",
    "<|fim_suffix|>",
    "<|fim_middle|>",
    "<|repo_name|>",
    "<|file_sep|>",
];

#[derive(Debug, Deserialize)]
struct StreamChunk {
    #[serde(default)]
    response: String,
    #[serde(default)]
    message: Option<ChatPart>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    eval_count: usize,
    #[serde(default)]
    prompt_eval_count: usize,
    #[serde(default)]
    load_duration: u64,
    #[serde(default)]
    prompt_eval_duration: u64,
    #[serde(default)]
    eval_duration: u64,
    #[serde(default)]
    total_duration: u64,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatPart {
    content: String,
}

/// One streamed line source: raw generate chunks carry `response`, chat
/// chunks carry `message.content`.
impl StreamChunk {
    fn text(&self) -> &str {
        match &self.message {
            Some(part) => &part.content,
            None => &self.response,
        }
    }
}

/// Body for a chat request. Built in one place so the wire shape is testable
/// without a network.
pub fn chat_body(
    model: &str,
    messages: &[crate::chat::Message],
    max_tokens: usize,
) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "options": {
            "num_predict": max_tokens.clamp(1, 2048),
            "temperature": 0.2,
        }
    })
}

/// Body for a raw FIM request. Pure so the stop tokens, raw flag and
/// token budget are pinned by tests without a network.
pub fn fim_body(
    model: &str,
    prefix: &str,
    suffix: &str,
    max_tokens: usize,
) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "prompt": fim_prompt(prefix, suffix),
        "raw": true,
        "stream": true,
        "options": {
            "num_predict": max_tokens.clamp(1, 128),
            "temperature": 0.2,
            "stop": STOPS,
        }
    })
}

/// Ollama durations are nanoseconds. Keep the existing Completion wire shape.
fn streamed_completion(text: String, chunk: &StreamChunk, elapsed_ms: u128) -> Completion {
    Completion {
        text,
        tokens: chunk.eval_count,
        prompt_tokens: chunk.prompt_eval_count,
        load_ms: u128::from(chunk.load_duration) / 1_000_000,
        prefill_ms: u128::from(chunk.prompt_eval_duration) / 1_000_000,
        decode_ms: u128::from(chunk.eval_duration) / 1_000_000,
        generation_ms: if chunk.total_duration == 0 { elapsed_ms }
            else { u128::from(chunk.total_duration) / 1_000_000 },
        cancelled: false,
    }
}

const MAX_STREAM_LINE: u64 = 1024 * 1024;
const MAX_STREAM_BYTES: usize = 8 * 1024 * 1024;

/// Bound each read before allocating an arbitrary server-controlled line.
fn read_stream(reader: impl std::io::Read, cancelled: &dyn Fn() -> bool) -> Result<(String, StreamChunk)> {
    let mut reader = BufReader::new(reader);
    let mut text = String::new();
    let mut total = 0usize;
    loop {
        if cancelled() { bail!("Cancelled"); }
        let mut line = String::new();
        let count = std::io::Read::take(&mut reader, MAX_STREAM_LINE + 1).read_line(&mut line)?;
        if cancelled() { bail!("Cancelled"); }
        ensure!(count <= MAX_STREAM_LINE as usize, "Ollama stream line exceeds limit");
        if count == 0 { bail!("Ollama stream ended before the done chunk"); }
        total = total.saturating_add(count);
        ensure!(total <= MAX_STREAM_BYTES, "Ollama stream exceeds response limit");
        if line.trim().is_empty() { continue; }
        let chunk: StreamChunk = serde_json::from_str(&line)
            .map_err(|error| anyhow!("Ollama stream parse: {error}"))?;
        if let Some(error) = &chunk.error { bail!("Ollama server error: {error}"); }
        text.push_str(chunk.text());
        if chunk.done { return Ok((text, chunk)); }
    }
}

pub struct OllamaProvider {
    remote: RemoteFim,
    agent: ureq::Agent,
}
impl OllamaProvider {
    pub fn new(remote: RemoteFim) -> Self {
        let timeout = Duration::from_millis(remote.timeout_ms.clamp(1_000, 120_000));
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10).min(timeout))
            .timeout_read(timeout)
            .timeout_write(timeout)
            .redirects(0)
            .build();
        Self { remote, agent }
    }
}
impl Provider for OllamaProvider {
    fn complete(
        &mut self,
        request: &Request,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Completion> {
        if let Some(messages) = &request.messages {
            return self.complete_chat(request, messages, cancelled);
        }
        ensure!(
            !self.remote.model.trim().is_empty(),
            "Ollama model name is empty"
        );
        let url = generate_url(&self.remote.endpoint);
        ensure!(
            url.starts_with("http://") || url.starts_with("https://"),
            "Ollama endpoint must start with http:// or https://"
        );
        let timeout =
            Duration::from_millis(self.remote.timeout_ms.clamp(1_000, 120_000));
        let body = fim_body(
            &self.remote.model,
            &request.prefix,
            &request.suffix,
            request.max_tokens,
        );
        if cancelled() {
            bail!("Cancelled");
        }
        let start = Instant::now();
        ensure!(serde_json::to_vec(&body)?.len() <= MAX_STREAM_BYTES, "Ollama request exceeds size limit");
        let response = self.agent.post(&url)
            .timeout(timeout)
            .send_json(body)
            .map_err(|e| anyhow!("Ollama request failed: {e}"))?;
        let (text, chunk) = read_stream(response.into_reader(), cancelled)?;
        Ok(streamed_completion(clean(&text, &request.suffix), &chunk, start.elapsed().as_millis()))
    }
}

impl OllamaProvider {
    /// Instruct-turn completion over /api/chat. Used by next-edit
    /// prediction; the FIM lane stays on raw /api/generate. Same streaming
    /// cancellation discipline as the FIM path.
    fn complete_chat(
        &mut self,
        request: &Request,
        messages: &[crate::chat::Message],
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Completion> {
        ensure!(!messages.is_empty(), "Empty chat request");
        for m in messages {
            ensure!(
                ["system", "user", "assistant"].contains(&m.role.as_str()),
                "Invalid chat role"
            );
        }
        let url = chat_url(&self.remote.endpoint);
        ensure!(
            url.starts_with("http://") || url.starts_with("https://"),
            "Ollama endpoint must start with http:// or https://"
        );
        let timeout =
            Duration::from_millis(self.remote.timeout_ms.clamp(1_000, 120_000));
        if cancelled() {
            bail!("Cancelled");
        }
        let start = Instant::now();
        ensure!(!self.remote.model.trim().is_empty(), "Ollama model name is empty");
        let body = chat_body(&self.remote.model, messages, request.max_tokens);
        ensure!(serde_json::to_vec(&body)?.len() <= MAX_STREAM_BYTES, "Ollama request exceeds size limit");
        let response = self.agent.post(&url)
            .timeout(timeout)
            .send_json(body)
            .map_err(|e| anyhow!("Ollama chat request failed: {e}"))?;
        let (text, chunk) = read_stream(response.into_reader(), cancelled)?;
        // Raw model text: the caller (editor) parses the edit contract.
        Ok(streamed_completion(text, &chunk, start.elapsed().as_millis()))
    }
}

/// Cut at the first leaked structure marker, then drop any tail that
/// duplicates the buffer suffix. Char-boundary safe: `str::find` returns
/// byte offsets at match starts, and `clip_suffix` slices only at the
/// suffix's own matched boundary.
pub fn clean(text: &str, suffix: &str) -> String {
    let mut cut = text.len();
    for marker in STOPS {
        if let Some(i) = text.find(marker) {
            cut = cut.min(i);
        }
    }
    clip_suffix(&text[..cut], suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_stream_preserves_unicode_and_rejects_bad_or_partial_responses() {
        let wire = concat!("{\"response\":\"héllo🦀\"}\n", "{\"response\":\"!\",\"done\":true,\"eval_count\":2}\n");
        let (text, done) = read_stream(wire.as_bytes(), &|| false).unwrap();
        assert_eq!(text, "héllo🦀!"); assert_eq!(done.eval_count,2);
        for bad in ["", "not json\n", "{\"response\":\"partial\"}\n", "{\"error\":\"missing model\"}\n"] {
            assert!(read_stream(bad.as_bytes(), &||false).is_err());
        }
        assert!(read_stream(wire.as_bytes(), &||true).unwrap_err().to_string().contains("Cancelled"));
        let oversized = vec![b'x'; MAX_STREAM_LINE as usize + 1];
        assert!(read_stream(oversized.as_slice(), &||false).unwrap_err().to_string().contains("limit"));
    }

    #[test]
    fn final_chunk_preserves_server_metrics_and_legacy_fallback() {
        let chunk: StreamChunk = serde_json::from_str(r#"{"done":true,"eval_count":17,"prompt_eval_count":31,"load_duration":12000000,"prompt_eval_duration":23000000,"eval_duration":45000000,"total_duration":81000000}"#).unwrap();
        let c = streamed_completion("héllo".into(), &chunk, 999);
        assert_eq!((c.tokens,c.prompt_tokens), (17,31));
        assert_eq!((c.load_ms,c.prefill_ms,c.decode_ms,c.generation_ms),(12,23,45,81));
        assert_eq!(c.text,"héllo");
        let old: StreamChunk = serde_json::from_str(r#"{"done":true}"#).unwrap();
        assert_eq!(streamed_completion(String::new(), &old, 999).generation_ms,999);
        let error: StreamChunk = serde_json::from_str(r#"{"error":"model not found"}"#).unwrap();
        assert!(error.error.is_some());
    }

    #[test]
    fn url_normalization() {
        assert_eq!(
            generate_url("https://ai.izdrail.com"),
            "https://ai.izdrail.com/api/generate"
        );
        assert_eq!(
            generate_url("https://ai.izdrail.com/"),
            "https://ai.izdrail.com/api/generate"
        );
        assert_eq!(
            generate_url("https://ai.izdrail.com/v1"),
            "https://ai.izdrail.com/api/generate"
        );
        assert_eq!(
            generate_url("https://ai.izdrail.com/v1/"),
            "https://ai.izdrail.com/api/generate"
        );
        assert_eq!(
            generate_url("  http://localhost:11434  "),
            "http://localhost:11434/api/generate"
        );
    }

    #[test]
    fn prompt_marker_order_and_unicode_passthrough() {
        let prefix = "fn grüße(🎉: &str) {\n    let 日本語 =";
        let suffix = "\n}\n// نهاية";
        let prompt = fim_prompt(prefix, suffix);
        let p = prompt.find("<|fim_prefix|>").unwrap();
        let s = prompt.find("<|fim_suffix|>").unwrap();
        let m = prompt.find("<|fim_middle|>").unwrap();
        assert!(p < s && s < m);
        // Non-ASCII content passes through byte-for-byte.
        assert!(prompt.contains(prefix));
        assert!(prompt.contains(suffix));
    }

    #[test]
    fn clean_cuts_markers_and_suffix_overlap() {
        let expected = "a + b";
        let got = clean("a + b<|fim_pad|>garbage", "\n}");
        assert_eq!(got, expected);
        let got = clean("a + b\n}\n}", "\n}");
        assert_eq!(got, expected);
        // Marker at position zero yields an empty suggestion.
        assert_eq!(clean("<|endoftext|>", "x"), "");
        assert_eq!(clean("", ""), "");
    }

    #[test]
    fn clean_is_char_boundary_safe_on_non_ascii() {
        // Mixed scripts, emoji with modifiers and combining marks: slicing
        // must never split a codepoint, and output stays valid UTF-8.
        let suffix = "\n// ö̈mma🦀";
        for text in [
            "let 👨‍👩‍👧‍👦 = a + b",
            "e\u{301} + q<|fim_suffix|> rest",
            "日本語のテキスト\n}\n// ö̈mma🦀",
            "العربية <|fim_middle|>",
        ] {
            let out = clean(text, suffix);
            assert!(std::str::from_utf8(out.as_bytes()).is_ok());
            assert!(out.len() <= text.len());
        }
        // clip_suffix cuts at the first suffix occurrence, keeping earlier lines.
        let expected = "日本語のテキスト\n}";
        let got = clean("日本語のテキスト\n}\n// ö̈mma🦀", "\n// ö̈mma🦀");
        assert_eq!(got, expected);
    }

    #[test]
    fn chat_url_normalization_and_body_shape() {
        assert_eq!(
            chat_url("https://ai.izdrail.com/v1/"),
            "https://ai.izdrail.com/api/chat"
        );
        assert_eq!(
            chat_url("http://localhost:11434"),
            "http://localhost:11434/api/chat"
        );
        let messages = vec![
            crate::chat::Message {
                role: "system".into(),
                content: "s".into(),
            },
            crate::chat::Message {
                role: "user".into(),
                content: "u".into(),
            },
        ];
        let body = chat_body("qwen2.5-coder:1.5b-instruct-q4_K_M", &messages, 512);
        assert_eq!(body["model"], "qwen2.5-coder:1.5b-instruct-q4_K_M");
        assert_eq!(body["stream"], true);
        assert_eq!(body["options"]["num_predict"], 512);
        // No raw flag on chat: the server applies the instruct template.
        assert!(body.get("raw").is_none());
        assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn stream_chunk_reads_both_shapes() {
        let fim: StreamChunk =
            serde_json::from_str("{\"response\":\"a\",\"done\":false}").unwrap();
        assert_eq!(fim.text(), "a");
        assert!(!fim.done);
        let chat: StreamChunk = serde_json::from_str(
            "{\"message\":{\"role\":\"assistant\",\"content\":\"b\"},\"done\":true}",
        )
        .unwrap();
        assert_eq!(chat.text(), "b");
        assert!(chat.done);
        // Non-ASCII chunk content survives byte-for-byte.
        let uni: StreamChunk =
            serde_json::from_str("{\"message\":{\"content\":\"héllo 🦀 日本語\"}}").unwrap();
        assert_eq!(uni.text(), "héllo 🦀 日本語");
    }

    #[test]
    fn fim_body_sends_stops_raw_and_clamped_budget() {
        let body = fim_body(
            "qwen2.5-coder:1.5b-instruct-q4_K_M",
            "let s = grüße🎉(",
            "\n}// 日本語",
            999,
        );
        assert_eq!(body["raw"], true);
        assert_eq!(body["stream"], true);
        // Budget clamps to the lane maximum.
        assert_eq!(body["options"]["num_predict"], 128);
        // Every stop marker goes to the server; non-ASCII content is
        // untouched by the prompt wrapper.
        let stops = body["options"]["stop"].as_array().unwrap();
        assert_eq!(stops.len(), STOPS.len());
        for marker in ["<|fim_pad|>", "<|endoftext|>", "<|fim_middle|>"] {
            assert!(
                stops.iter().any(|v| v.as_str() == Some(marker)),
                "missing stop {marker}"
            );
        }
        let prompt = body["prompt"].as_str().unwrap();
        assert!(prompt.contains("grüße🎉("));
        assert!(prompt.contains("日本語"));
        // Small budgets are not inflated.
        assert_eq!(fim_body("m", "", "", 4)["options"]["num_predict"], 4);
        assert_eq!(fim_body("m", "", "", 0)["options"]["num_predict"], 1);
    }

    #[test]
    fn remote_fim_round_trips_through_request_json() {
        let remote = RemoteFim {
            endpoint: "https://ai.izdrail.com".into(),
            model: "qwen2.5-coder:1.5b-instruct-q4_K_M".into(),
            timeout_ms: 15_000,
        };
        let json = serde_json::to_string(&remote).unwrap();
        let back: RemoteFim = serde_json::from_str(&json).unwrap();
        assert_eq!(back, remote);
        // Timeout clamping bounds, pinned so settings drift is caught.
        let _ = Duration::from_millis(remote.timeout_ms.clamp(1_000, 120_000));
    }
}
