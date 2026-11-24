//! Optional local llama.cpp backend. No downloads, no implicit GPU offload.
use anyhow::{Result, anyhow, ensure};
use llama_cpp_2::{context::params::LlamaContextParams, llama_backend::LlamaBackend,
    llama_batch::LlamaBatch, model::{LlamaModel, params::LlamaModelParams}, sampling::LlamaSampler};
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
        ensure!(request.messages.is_none(), "llama.cpp chat templates are not implemented; use Candle or Ollama chat");
        ensure!((1..=128).contains(&request.max_tokens), "FIM output limit must be 1..128");
        ensure!((1..=2048).contains(&request.prefix_tokens) && request.suffix_tokens <= 1024, "Invalid FIM token budget");
        ensure!(!cancelled(), "Cancelled");
        let vocab = self.model.vocab();
        // Require each FIM marker to be an actual special token, rather than
        // treating a generic chat model's ordinary marker text as FIM support.
        let marker = |s: &str| -> Result<_> {
            let special = vocab.tokenize(s.as_bytes(), false, true);
            let ordinary = vocab.tokenize(s.as_bytes(), false, false);
            ensure!(special.len() == 1 && special != ordinary, "Model lacks required FIM special tokens");
            Ok(special[0])
        };
        let prefix_marker = marker("<|fim_prefix|>")?;
        let suffix_marker = marker("<|fim_suffix|>")?;
        let middle_marker = marker("<|fim_middle|>")?;
        let prefix = vocab.tokenize(request.prefix.as_bytes(), false, false);
        let suffix = vocab.tokenize(request.suffix.as_bytes(), false, false);
        let mut prompt = vec![prefix_marker];
        prompt.extend_from_slice(&prefix[prefix.len().saturating_sub(request.prefix_tokens)..]);
        prompt.push(suffix_marker);
        prompt.extend_from_slice(&suffix[..suffix.len().min(request.suffix_tokens)]);
        prompt.push(middle_marker);
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
        let mut sampler = LlamaSampler::greedy();
        let mut generated = Vec::new();
        for _ in 0..request.max_tokens {
            ensure!(!cancelled(), "Cancelled");
            let token = sampler.sample(&context, batch.n_tokens() - 1);
            if vocab.is_eog(token) || [prefix_marker, suffix_marker, middle_marker].contains(&token) { break; }
            sampler.accept(token);
            generated.push(token);
            if generated.len() == request.max_tokens { break; }
            batch.clear();
            batch.add(token, (prompt.len() + generated.len() - 1) as i32, &[0], true)?;
            context.decode(&mut batch)?;
        }
        let text = String::from_utf8(vocab.detokenize(&generated, true, false))?;
        let generation_ms = start.elapsed().as_millis();
        Ok(Completion { text: crate::ollama::clean(&text, &request.suffix), tokens: generated.len(),
            prompt_tokens: prompt.len(), load_ms: self.load_ms, generation_ms, prefill_ms,
            decode_ms: generation_ms.saturating_sub(prefill_ms), cancelled: false })
    }
}
