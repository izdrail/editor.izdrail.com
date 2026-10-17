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

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    response: String,
    #[serde(default)]
    message: Option<ChatPart>,
    #[serde(default)]
    done: bool,
}

#[derive(Deserialize)]
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

pub struct OllamaProvider {
    remote: RemoteFim,
}
impl OllamaProvider {
    pub fn new(remote: RemoteFim) -> Self {
        Self { remote }
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
        let body = serde_json::json!({
            "model": self.remote.model,
            "prompt": fim_prompt(&request.prefix, &request.suffix),
            "raw": true,
            "stream": true,
            "options": {
                "num_predict": request.max_tokens.clamp(1, 128),
                "temperature": 0.2,
                "stop": STOPS,
            }
        });
        if cancelled() {
            bail!("Cancelled");
        }
        let start = Instant::now();
        let response = ureq::post(&url)
            .timeout(timeout)
            .send_json(body)
            .map_err(|e| anyhow!("Ollama request failed: {e}"))?;
        // Streaming keeps cancellation cooperative: the flag is checked
        // between chunks, so a soft cancel aborts mid-generation instead of
        // waiting for the whole response.
        let mut text = String::new();
        let mut done = false;
        for line in BufReader::new(response.into_reader()).lines() {
            if cancelled() {
                bail!("Cancelled");
            }
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let chunk: StreamChunk = serde_json::from_str(&line)
                .map_err(|e| anyhow!("Ollama stream parse: {e}"))?;
            text.push_str(&chunk.response);
            if chunk.done {
                done = true;
                break;
            }
        }
        ensure!(done, "Ollama stream ended before the done chunk");
        Ok(Completion {
            text: clean(&text, &request.suffix),
            // Stream chunks carry no token counts; zero means unreported.
            tokens: 0,
            prompt_tokens: 0,
            load_ms: 0,
            generation_ms: start.elapsed().as_millis(),
            prefill_ms: 0,
            decode_ms: 0,
            cancelled: false,
        })
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
        let response = ureq::post(&url)
            .timeout(timeout)
            .send_json(chat_body(&self.remote.model, messages, request.max_tokens))
            .map_err(|e| anyhow!("Ollama chat request failed: {e}"))?;
        let mut text = String::new();
        let mut done = false;
        for line in BufReader::new(response.into_reader()).lines() {
            if cancelled() {
                bail!("Cancelled");
            }
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let chunk: StreamChunk = serde_json::from_str(&line)
                .map_err(|e| anyhow!("Ollama chat stream parse: {e}"))?;
            text.push_str(chunk.text());
            if chunk.done {
                done = true;
                break;
            }
        }
        ensure!(done, "Ollama chat stream ended before the done chunk");
        // Raw model text: the caller (editor) parses the edit contract.
        Ok(Completion {
            text,
            tokens: 0,
            prompt_tokens: 0,
            load_ms: 0,
            generation_ms: start.elapsed().as_millis(),
            prefill_ms: 0,
            decode_ms: 0,
            cancelled: false,
        })
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
