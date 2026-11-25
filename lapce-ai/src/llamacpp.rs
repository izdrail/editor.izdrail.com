//! Optional local llama.cpp backend. No downloads, no implicit GPU offload.
use anyhow::{Result, anyhow, ensure};
use llama_cpp_2::{context::params::LlamaContextParams, llama_backend::LlamaBackend,
    llama_batch::LlamaBatch, model::{LlamaModel, LlamaChatMessage, params::LlamaModelParams}, sampling::LlamaSampler};
use std::{num::NonZeroU32, path::Path, sync::OnceLock, time::Instant};
use crate::{Completion, Provider, Request};

static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
fn backend() -> Result<&'static LlamaBackend> {
    if BACKEND.get().is_none() {
        let initialized = LlamaBackend::init()?;
        let _ = BACKEND.set(initialized);
    }
    BACKEND.get().ok_or_else(|| anyhow!("llama.cpp backend initialization failed"))
}

/// Explicit per-lane native sampling. Absent config keeps legacy greedy.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SamplingConfig {
    #[serde(default)]
    temperature: f32,
    top_k: Option<i32>,
    top_p: Option<f32>,
    #[serde(default)]
    seed: u32,
}
impl SamplingConfig {
    fn parse(value: &str) -> Result<Self> {
        let config: Self = serde_json::from_str(value)?;
        ensure!(config.temperature.is_finite() && (0.0..=5.0).contains(&config.temperature), "llama.cpp temperature must be 0..5");
        ensure!(config.top_k.is_none_or(|k| (1..=1_000_000).contains(&k)), "llama.cpp top_k must be positive");
        ensure!(config.top_p.is_none_or(|p| p.is_finite() && p > 0.0 && p <= 1.0), "llama.cpp top_p must be >0 and <=1");
        ensure!(config.temperature > 0.0 || (config.top_k.is_none() && config.top_p.is_none()), "llama.cpp greedy does not accept top_k/top_p");
        Ok(config)
    }
    fn sampler(&self) -> LlamaSampler {
        if self.temperature == 0.0 { return LlamaSampler::greedy(); }
        let mut chain = Vec::new();
        if let Some(k) = self.top_k { chain.push(LlamaSampler::top_k(k)); }
        if let Some(p) = self.top_p { chain.push(LlamaSampler::top_p(p, 1)); }
        chain.push(LlamaSampler::temp(self.temperature));
        chain.push(LlamaSampler::dist(self.seed));
        LlamaSampler::chain_simple(chain)
    }
}
fn sampler_for_lane(chat: bool) -> Result<LlamaSampler> {
    let name = if chat { "LAPCE_AI_LLAMA_CHAT_SAMPLING" } else { "LAPCE_AI_LLAMA_FIM_SAMPLING" };
    match std::env::var(name) {
        Ok(value) => Ok(SamplingConfig::parse(&value)?.sampler()),
        Err(std::env::VarError::NotPresent) => Ok(LlamaSampler::greedy()),
        Err(error) => Err(error.into()),
    }
}

pub struct LlamaCppProvider {
    model: LlamaModel,
    load_ms: u128,
}
impl LlamaCppProvider {
    pub fn load(path: &Path) -> Result<Self> {
        let start = Instant::now();
        let layers: u32 = std::env::var("LAPCE_AI_GPU_LAYERS").unwrap_or_else(|_| "0".into()).parse()?;
        ensure!(layers == 0 || cfg!(feature = "llama-cpp-cuda"), "GPU layers requested without llama-cpp-cuda");
        let mut params = std::pin::pin!(LlamaModelParams::default().with_n_gpu_layers(layers));
        let cpu_experts = std::env::var("LAPCE_AI_CPU_EXPERTS").unwrap_or_else(|_| "0".into());
        ensure!(["0", "1"].contains(&cpu_experts.as_str()), "LAPCE_AI_CPU_EXPERTS must be 0 or 1");
        if cpu_experts == "1" { params.as_mut().add_cpu_moe_override(); }
        let model = LlamaModel::load_from_file(backend()?, path, &params)?;
        eprintln!("AI backend=llama.cpp requested_gpu_layers={layers}; verify runtime offload in native logs");
        Ok(Self { model, load_ms: start.elapsed().as_millis() })
    }
}
impl Provider for LlamaCppProvider {
    fn complete(&mut self, request: &Request, cancelled: &dyn Fn() -> bool) -> Result<Completion> {
        ensure!(request.remote.is_none(), "llama.cpp is a local-only provider");
        let chat = request.messages.is_some();
        let mut sampler = sampler_for_lane(chat)?;
        ensure!((1..=if chat {512} else {128}).contains(&request.max_tokens), "Invalid output token limit");
        ensure!(!cancelled(), "Cancelled");
        let vocab = self.model.vocab();
        let mut fim_stops = Vec::new();
        let prompt = if let Some(messages) = &request.messages {
            ensure!(!messages.is_empty(), "Empty conversation");
            let messages = messages.iter().map(|message| {
                ensure!(["system", "user", "assistant"].contains(&message.role.as_str()), "Invalid chat role");
                // Match the existing engine's reserved-marker escaping.
                Ok(LlamaChatMessage::new(message.role.clone(), message.content.replace("<|", "< | "))?)
            }).collect::<Result<Vec<_>>>()?;
            let template = self.model.chat_template(None)?;
            let formatted = self.model.apply_chat_template(&template, &messages, true)?;
            vocab.tokenize(formatted.as_bytes(), true, true)
        } else {
            ensure!((1..=2048).contains(&request.prefix_tokens) && request.suffix_tokens <= 1024, "Invalid FIM token budget");
            let marker = |s: &str| -> Result<_> {
                let special = vocab.tokenize(s.as_bytes(), false, true);
                let ordinary = vocab.tokenize(s.as_bytes(), false, false);
                ensure!(special.len() == 1 && special != ordinary, "Model lacks required FIM special tokens");
                Ok(special[0])
            };
            let prefix_marker = marker("<|fim_prefix|>")?;
            let suffix_marker = marker("<|fim_suffix|>")?;
            let middle_marker = marker("<|fim_middle|>")?;
            fim_stops.extend([prefix_marker, suffix_marker, middle_marker]);
            let prefix = vocab.tokenize(request.prefix.as_bytes(), false, false);
            let suffix = vocab.tokenize(request.suffix.as_bytes(), false, false);
            let mut prompt = vec![prefix_marker];
            prompt.extend_from_slice(&prefix[prefix.len().saturating_sub(request.prefix_tokens)..]);
            prompt.push(suffix_marker);
            prompt.extend_from_slice(&suffix[..suffix.len().min(request.suffix_tokens)]);
            prompt.push(middle_marker);
            prompt
        };
        ensure!(!prompt.is_empty(), "Empty tokenized prompt");
        ensure!(!cancelled(), "Cancelled");
        let mut context = self.model.new_context(backend()?, LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(4096)).with_n_batch(512).with_n_ubatch(512))?;
        ensure!(prompt.len() + request.max_tokens <= context.n_ctx() as usize, "FIM context exceeds configured llama.cpp context");
        let start = Instant::now();
        let mut batch = LlamaBatch::new(512, 1);
        for (chunk_index, chunk) in prompt.chunks(512).enumerate() {
            ensure!(!cancelled(), "Cancelled");
            batch.clear();
            for (i, &token) in chunk.iter().enumerate() {
                let position = chunk_index * 512 + i;
                batch.add(token, position as i32, &[0], position + 1 == prompt.len())?;
            }
            context.decode(&mut batch)?;
        }
        let prefill_ms = start.elapsed().as_millis();
        let mut generated = Vec::new();
        for _ in 0..request.max_tokens {
            ensure!(!cancelled(), "Cancelled");
            let token = sampler.sample(&context, batch.n_tokens() - 1);
            if vocab.is_eog(token) || fim_stops.contains(&token) { break; }
            sampler.accept(token);
            generated.push(token);
            if generated.len() == request.max_tokens { break; }
            batch.clear();
            batch.add(token, (prompt.len() + generated.len() - 1) as i32, &[0], true)?;
            context.decode(&mut batch)?;
        }
        let text = String::from_utf8(vocab.detokenize(&generated, true, false))?;
        let generation_ms = start.elapsed().as_millis();
        Ok(Completion { text: if chat { text } else { crate::ollama::clean(&text, &request.suffix) }, tokens: generated.len(),
            prompt_tokens: prompt.len(), load_ms: self.load_ms, generation_ms, prefill_ms,
            decode_ms: generation_ms.saturating_sub(prefill_ms), cancelled: false })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampling_profiles_validate_without_loading_weights() {
        for input in [r#"{"min_p":0.1}"#, r#"{"temperature":-1}"#, r#"{"top_k":0}"#, r#"{"temperature":0,"top_p":0.9}"#, r#"{"temperature":1,"top_p":2}"#] {
            assert!(SamplingConfig::parse(input).is_err(), "{input}");
        }
        let _greedy = SamplingConfig::parse("{}").unwrap().sampler();
        let _sampled = SamplingConfig::parse(r#"{"temperature":0.7,"top_k":20,"top_p":0.9,"seed":42}"#).unwrap().sampler();
    }

    #[test]
    #[ignore = "requires exact local GGUF via AI_MODEL; no implicit download"]
    fn real_model_chat_and_cancellation() {
        let model = std::path::PathBuf::from(std::env::var("AI_MODEL").unwrap());
        let mut provider = LlamaCppProvider::load(&model).unwrap();
        let request = Request {
            model, tokenizer: Default::default(), prefix: String::new(), suffix: String::new(),
            prefix_tokens: 128, suffix_tokens: 64, max_tokens: 64, request_id: 1,
            messages: Some(vec![crate::chat::Message {role: "user".into(), content: "Reply with a short greeting.".into()}]),
            remote: None,
        };
        assert!(provider.complete(&request, &|| true).unwrap_err().to_string().contains("Cancelled"));
        for _ in 0..3 {
            let output = provider.complete(&request, &|| false).unwrap();
            assert!(!output.text.trim().is_empty(), "Empty model response");
            assert!(output.tokens > 0 && output.prompt_tokens > 0);
            assert!(!output.cancelled);
            eprintln!("{}", serde_json::to_string(&output).unwrap());
        }
    }
}
