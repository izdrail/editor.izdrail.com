//! Qwen FIM providers: local CPU-only Candle (no network) and an
//! explicit, config-carried Ollama network lane. No implicit downloads;
//! remote inference happens only when a request carries remote parameters.
pub mod capabilities;
pub mod catalog;
pub mod chat;
pub mod discovery;
pub mod ollama;
#[cfg(feature = "llama-cpp")]
pub mod llamacpp;
pub mod process;
pub mod sampling;
pub mod mistralrs;
use anyhow::{Result, anyhow, bail, ensure};
use candle_core::{Device, Tensor, quantized::gguf_file};
use candle_transformers::models::quantized_qwen2::ModelWeights;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufRead, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};
use tokenizers::Tokenizer;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub model: PathBuf,
    pub tokenizer: PathBuf,
    pub prefix: String,
    pub suffix: String,
    pub prefix_tokens: usize,
    pub suffix_tokens: usize,
    pub max_tokens: usize,
    /// Worker-side job identity. Set by the dispatcher, not by the editor:
    /// cancellation lines reference it so a stale cancel can never kill a
    /// newer request. Zero means untagged (older client) and matches any
    /// in-flight request.
    #[serde(default)]
    pub request_id: u64,
    #[serde(default)]
    pub messages: Option<Vec<chat::Message>>,
    /// Network FIM lane: when set, the worker POSTs to this Ollama endpoint
    /// instead of loading local model files; model/tokenizer are ignored.
    /// Absent in older clients, so the default keeps the local lane.
    #[serde(default)]
    pub remote: Option<ollama::RemoteFim>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Completion {
    pub text: String,
    pub tokens: usize,
    pub prompt_tokens: usize,
    pub load_ms: u128,
    pub generation_ms: u128,
    pub prefill_ms: u128,
    pub decode_ms: u128,
    /// True when generation stopped early because a cancellation line named
    /// this request. The worker still emits exactly one response line so the
    /// parent pipe stays in sync; text is empty.
    #[serde(default)]
    pub cancelled: bool,
}
/// One stdin line for the worker process: either a completion request or a
/// cooperative cancellation naming the in-flight request id. Cancellation
/// keeps the process (and the loaded model) alive; killing the process
/// remains the hard fallback.
#[derive(Debug)]
pub enum Wire {
    Request(Request),
    Cancel(u64),
}
#[derive(Deserialize)]
struct CancelLine {
    cancel_request: u64,
}
pub fn parse_wire(line: &str) -> Result<Wire> {
    if let Ok(cancel) = serde_json::from_str::<CancelLine>(line) {
        return Ok(Wire::Cancel(cancel.cancel_request));
    }
    Ok(Wire::Request(serde_json::from_str(line)?))
}
/// Parent side of the cancellation line. Requests and cancellations share one
/// stdin, so a cancel is only ever meaningful for the request id it names.
pub fn cancel_line(request_id: u64) -> String {
    format!("{{\"cancel_request\":{request_id}}}")
}
/// True when an error means cooperative cancellation, not a failure.
fn is_cancelled(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| cause.to_string() == "Cancelled")
}

pub trait Provider {
    fn complete(
        &mut self,
        request: &Request,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Completion>;
}
pub struct CandleProvider {
    model: ModelWeights,
    tokenizer: Tokenizer,
    load_ms: u128,
    context_length: usize,
    device: Device,
}
// Construct special marker spellings in code rather than shell transport.
fn marker(name: &str) -> String {
    format!("<|{name}|>")
}
fn token(tokenizer: &Tokenizer, name: &str) -> Result<u32> {
    tokenizer
        .token_to_id(&marker(name))
        .ok_or_else(|| anyhow!("Tokenizer lacks {name}"))
}
pub fn prompt_ids(tokenizer: &Tokenizer, request: &Request) -> Result<Vec<u32>> {
    if let Some(messages) = &request.messages {
        return chat::prompt_ids(tokenizer, messages, request.max_tokens);
    }
    ensure!(
        (1..=2048).contains(&request.prefix_tokens),
        "prefix tokens must be 1..2048"
    );
    ensure!(
        request.suffix_tokens <= 1024,
        "suffix tokens must be <=1024"
    );
    ensure!(
        (1..=128).contains(&request.max_tokens),
        "max tokens must be 1..128"
    );
    let encode = |s: &str| {
        tokenizer
            .encode(s, false)
            .map_err(|e| anyhow!(e.to_string()))
    };
    let prefix = encode(&request.prefix)?;
    let suffix = encode(&request.suffix)?;
    let p = prefix.get_ids();
    let s = suffix.get_ids();
    let mut ids = vec![token(tokenizer, "fim_prefix")?];
    ids.extend_from_slice(&p[p.len().saturating_sub(request.prefix_tokens)..]);
    ids.push(token(tokenizer, "fim_suffix")?);
    ids.extend_from_slice(&s[..s.len().min(request.suffix_tokens)]);
    ids.push(token(tokenizer, "fim_middle")?);
    Ok(ids)
}
pub fn greedy(logits: &[f32]) -> Result<u32> {
    logits
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_finite())
        .max_by(|(ia, a), (ib, b)| a.total_cmp(b).then_with(|| ib.cmp(ia)))
        .map(|(i, _)| i as u32)
        .ok_or_else(|| anyhow!("No finite logits"))
}
/// Small deterministic loop shared by inference and tests. Stop markers are never emitted.
pub fn sample_loop(
    max: usize,
    stops: &[u32],
    cancelled: &dyn Fn() -> bool,
    mut next: impl FnMut(&[u32]) -> Result<u32>,
) -> Result<Vec<u32>> {
    let mut out = Vec::new();
    for _ in 0..max {
        if cancelled() {
            bail!("Cancelled");
        }
        let id = next(&out)?;
        if stops.contains(&id) {
            break;
        }
        out.push(id);
    }
    Ok(out)
}
/// CPU is unchanged by default. `cuda` fails closed; only explicitly chosen
/// `auto` may fall back, and reports that decision on stderr, never JSON stdout.
fn selected_device() -> Result<Device> {
    let mode = std::env::var("LAPCE_AI_DEVICE").unwrap_or_else(|_| "cpu".into());
    ensure!(["cpu", "auto", "cuda"].contains(&mode.as_str()),
        "LAPCE_AI_DEVICE must be cpu, auto or cuda");
    if mode == "cpu" {
        eprintln!("AI backend=candle selected_device=cpu");
        return Ok(Device::Cpu);
    }
    match Device::new_cuda(0) {
        Ok(device) => {
            eprintln!("AI backend=candle selected_device=cuda:0");
            Ok(device)
        }
        Err(_) if mode == "auto" => {
            eprintln!("AI backend=candle selected_device=cpu fallback=cuda_unavailable");
            Ok(Device::Cpu)
        }
        Err(_) => bail!("CUDA requested but unavailable; compile candle-cuda and check the local driver/runtime"),
    }
}

/// Reject incompatible external tokenizer IDs before any weight allocation.
/// Equal vocabulary sizes alone do not establish token-ID compatibility.
fn validate_vocabulary(
    vocab: &std::collections::HashMap<String, u32>,
    model_vocab: usize,
    gguf_tokens: Option<&Vec<gguf_file::Value>>,
) -> Result<()> {
    ensure!(model_vocab > 0 && !vocab.is_empty(), "Empty model/tokenizer vocabulary");
    for (text, &id) in vocab {
        ensure!((id as usize) < model_vocab, "Tokenizer ID exceeds model vocabulary");
        if let Some(tokens) = gguf_tokens {
            ensure!(tokens.get(id as usize)
                .and_then(|v| v.to_string().ok()).is_some_and(|v| v == text),
                "Tokenizer token IDs do not match GGUF vocabulary");
        }
    }
    Ok(())
}

impl CandleProvider {
    pub fn load(model: &Path, tokenizer: &Path) -> Result<Self> {
        Self::load_with_device(model, tokenizer, Device::Cpu)
    }

    /// Explicit device selection, preserving the legacy CPU load() default.
    pub fn load_with_device(model: &Path, tokenizer: &Path, device: Device) -> Result<Self> {
        let start = Instant::now();
        let mut reader = File::open(model)?;
        let content = gguf_file::Content::read(&mut reader)?;
        ensure!(
            content
                .metadata
                .get("general.architecture")
                .and_then(|v| v.to_string().ok())
                == Some(&"qwen2".to_owned()),
            "Only qwen2 GGUF is supported"
        );
        let context_length = content.metadata.get("qwen2.context_length")
            .ok_or_else(|| anyhow!("Model lacks qwen2 context length"))?.to_u32()? as usize;
        ensure!(context_length > 0, "Model context length is zero");
        let vocab_size = content.tensor_infos.get("token_embd.weight")
            .and_then(|info| info.shape.dims().first()).copied()
            .ok_or_else(|| anyhow!("Model lacks token embeddings"))?;
        let tokenizer =
            Tokenizer::from_file(tokenizer).map_err(|e| anyhow!(e.to_string()))?;
        let gguf_tokens = content.metadata.get("tokenizer.ggml.tokens")
            .map(|v| v.to_vec()).transpose()?;
        validate_vocabulary(&tokenizer.get_vocab(true), vocab_size, gguf_tokens)?;
        for name in ["endoftext"] {
            token(&tokenizer, name)?;
        }
        let model = ModelWeights::from_gguf(content, &mut reader, &device)?;
        Ok(Self {
            model,
            tokenizer,
            load_ms: start.elapsed().as_millis(),
            context_length,
            device,
        })
    }
}
impl Provider for CandleProvider {
    fn complete(
        &mut self,
        request: &Request,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Completion> {
        ensure!(!cancelled(), "Cancelled");
        let ids = prompt_ids(&self.tokenizer, request)?;
        ensure!(!cancelled(), "Cancelled");
        ensure!(ids.len().checked_add(request.max_tokens)
            .is_some_and(|n| n <= self.context_length),
            "Prompt and output exceed the model context length");
        let mut stops = [
            "fim_prefix",
            "fim_suffix",
            "fim_middle",
            "fim_pad",
            "repo_name",
            "file_sep",
            "endoftext",
            "im_end",
        ]
        .iter()
        .filter_map(|name| self.tokenizer.token_to_id(&marker(name)))
        .collect::<Vec<_>>();
        let config = sampling::from_environment(request.messages.is_some())?;
        let mut sampler = config.as_ref().map(|c| c.processor(self.tokenizer.get_vocab_size(true))).transpose()?.flatten();
        if let Some(config) = &config { stops.extend_from_slice(&config.stop_token_ids); }
        let start = Instant::now();
        // Candle 0.9.1 resets its per-layer KV cache when index_pos is zero.
        let mut prefill_ms = 0;
        let generated = sample_loop(request.max_tokens, &stops, cancelled, |out| {
            let (input, pos) = if out.is_empty() {
                (ids.as_slice(), 0)
            } else {
                (&out[out.len() - 1..], ids.len() + out.len() - 1)
            };
            let step = Instant::now();
            let input = Tensor::new(input, &self.device)?.unsqueeze(0)?;
            let mut logits = self
                .model
                .forward(&input, pos)?
                .squeeze(0)?
                .to_vec1::<f32>()?;
            if out.is_empty() {
                prefill_ms = step.elapsed().as_millis();
            }
            if let Some(config) = &config { config.penalize(&mut logits, out)?; }
            if let Some(sampler) = sampler.as_mut() {
                ensure!(logits.iter().all(|v| v.is_finite()), "Non-finite logits cannot be sampled");
                Ok(sampler.sample(&Tensor::new(logits.as_slice(), &Device::Cpu)?)?)
            } else { greedy(&logits) }
        })?;
        let text = self
            .tokenizer
            .decode(&generated, true)
            .map_err(|e| anyhow!(e.to_string()))?;
        let generation_ms = start.elapsed().as_millis();
        Ok(Completion {
            text: if request.messages.is_some() {
                text
            } else {
                clip_suffix(&text, &request.suffix)
            },
            tokens: generated.len(),
            prompt_tokens: ids.len(),
            load_ms: self.load_ms,
            generation_ms,
            prefill_ms,
            decode_ms: generation_ms.saturating_sub(prefill_ms),
            cancelled: false,
        })
    }
}

/// Drop old weights before allocating a replacement, avoiding double-model
/// peak memory. A failed load leaves the cache empty, never a stale model.
fn replace_loaded<T>(slot: &mut Option<T>, replace: bool, cancelled: &dyn Fn() -> bool, load: impl FnOnce() -> Result<T>) -> Result<()> {
    ensure!(!cancelled(), "Cancelled");
    if replace {
        drop(slot.take());
        *slot = Some(load()?);
    }
    ensure!(!cancelled(), "Cancelled");
    Ok(())
}

/// Do not insert the existing suffix again when a small model runs past the hole.
pub fn clip_suffix(text: &str, suffix: &str) -> String {
    if suffix.len() >= 2 && !suffix.trim().is_empty() {
        if let Some(at) = text.find(suffix) {
            return text[..at].to_string();
        }
    }
    text.to_string()
}

pub const MODEL_REV: &str = "73c3c696087d8d57a3005e31f2afe78f43059739";
pub const TOKEN_REV: &str = "df3ce67c0e24480f20468b6ef2894622d69eb73b";
pub const MODEL_FILE: &str = "Qwen2.5-Coder-1.5B.Q4_K_M.gguf";
pub const MODEL_HASH: &str =
    "a9ccb6be802412e61e1acbee6287a23b11e56e94718560f910075c64b6194910";
pub const TOKEN_HASH: &str =
    "c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539";
pub fn verify(path: &Path, size: u64, expected: &str) -> Result<()> {
    let mut file = File::open(path)?;
    ensure!(file.metadata()?.len() == size, "Wrong file size");
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    ensure!(
        format!("{:x}", hash.finalize()) == expected,
        "Checksum mismatch"
    );
    Ok(())
}
#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadEvent {
    pub state: String,
    pub bytes: usize,
    pub total: usize,
    pub message: String,
}
struct Progress {
    bytes: usize,
    total: usize,
    last: usize,
}
fn event(state: &str, bytes: usize, total: usize, message: &str) {
    println!(
        "{}",
        serde_json::to_string(&DownloadEvent {
            state: state.into(),
            bytes,
            total,
            message: message.into()
        })
        .unwrap()
    );
    let _ = std::io::stdout().flush();
}
impl hf_hub::api::Progress for Progress {
    fn init(&mut self, size: usize, name: &str) {
        self.total = size;
        event("downloading", self.bytes, size, name);
    }
    fn update(&mut self, size: usize) {
        self.bytes += size;
        if self.bytes - self.last >= 4 * 1024 * 1024 {
            event("downloading", self.bytes, self.total, "");
            self.last = self.bytes;
        }
    }
    fn finish(&mut self) {
        event("verifying", self.bytes, self.total, "");
    }
}
#[derive(Serialize, Deserialize)]
pub struct Installed {
    pub model: PathBuf,
    pub tokenizer: PathBuf,
}
pub fn download(root: &Path) -> Result<Installed> {
    download_model(root, "qwen-fim-1.5b")
}
pub fn download_model(root: &Path, id: &str) -> Result<Installed> {
    let entry = catalog::find(id).ok_or_else(|| anyhow!("Unknown curated model"))?;
    use hf_hub::{Repo, RepoType, api::sync::ApiBuilder};
    std::fs::create_dir_all(root)?;
    let api = ApiBuilder::new()
        .with_cache_dir(root.join("hub"))
        .with_token(None)
        .with_progress(false)
        .build()?;
    // Explicit download calls use callbacks; cached immutable paths are checked independently.
    let obtain_progress =
        |repo: &str, rev: &str, name: &str, size, hash| -> Result<PathBuf> {
            let handle = api.repo(Repo::with_revision(
                repo.into(),
                RepoType::Model,
                rev.into(),
            ));
            let cached = hf_hub::Cache::new(root.join("hub"))
                .repo(Repo::with_revision(
                    repo.into(),
                    RepoType::Model,
                    rev.into(),
                ))
                .get(name);
            let path = if let Some(path) = cached {
                path
            } else {
                handle.download_with_progress(
                    name,
                    Progress {
                        bytes: 0,
                        total: 0,
                        last: 0,
                    },
                )?
            };
            verify(&path, size, hash)?;
            Ok(path)
        };
    let model = obtain_progress(
        entry.repo,
        entry.revision,
        entry.file,
        entry.size,
        entry.hash,
    )?;
    let tokenizer = obtain_progress(
        "Qwen/Qwen2.5-Coder-1.5B-Instruct",
        chat::TOKEN_REV,
        "tokenizer.json",
        7031645,
        TOKEN_HASH,
    )?;
    let installed = Installed { model, tokenizer };
    let manifest = format!("installed-{}.json", entry.id);
    let temp = root.join(format!("{manifest}.part"));
    std::fs::write(&temp, serde_json::to_vec_pretty(&installed)?)?;
    std::fs::rename(temp, root.join(&manifest))?;
    if entry.id == "qwen-fim-1.5b" {
        std::fs::write(
            root.join("installed.json"),
            serde_json::to_vec_pretty(&installed)?,
        )?;
    }
    event("ready", 0, 0, entry.id);
    Ok(installed)
}
/// Cooperative cancellation needs a control thread: stdin carries both
/// requests and cancellation lines, but inference blocks the serving thread,
/// so it could never read a cancellation from the same stream in time.
pub fn run() -> i32 {
    let result = (|| -> Result<()> {
        if std::env::args().nth(1).as_deref() == Some("--ai-ollama-models") {
            let endpoint = std::env::args().nth(2)
                .ok_or_else(|| anyhow!("Missing explicit Ollama endpoint"))?;
            let model = std::env::args().nth(3);
            println!("{}", discovery::discover(&endpoint, model.as_deref())?);
            return Ok(());
        }
        if std::env::args().nth(1).as_deref() == Some("--ai-capabilities") {
            println!("{}", serde_json::to_string(&capabilities::available())?);
            return Ok(());
        }
        if std::env::args().nth(1).as_deref() == Some("--ai-download") {
            let root = std::env::args()
                .nth(2)
                .ok_or_else(|| anyhow!("Missing download directory"))?;
            let id = std::env::args()
                .nth(3)
                .unwrap_or_else(|| "qwen-fim-1.5b".into());
            download_model(Path::new(&root), &id)?;
            return Ok(());
        }
        use std::sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        };
        let cancel_for = Arc::new(AtomicU64::new(u64::MAX));
        let (tx, rx) = std::sync::mpsc::channel::<Request>();
        std::thread::spawn({
            let cancel_for = cancel_for.clone();
            move || {
                let stdin = std::io::stdin();
                for line in stdin.lock().lines() {
                    let Ok(line) = line else { return };
                    match parse_wire(&line) {
                        Ok(Wire::Cancel(id)) => {
                            cancel_for.store(id, Ordering::Release);
                        }
                        Ok(Wire::Request(request)) => {
                            if tx.send(request).is_err() {
                                return;
                            }
                        }
                        Err(e) => {
                            eprintln!("AI worker: bad input line: {e}");
                        }
                    }
                }
            }
        });
        let mut provider: Option<(PathBuf, PathBuf, CandleProvider)> = None;
        let mut remote_provider: Option<ollama::OllamaProvider> = None;
        #[cfg(feature = "llama-cpp")]
        let mut llama_provider: Option<(PathBuf, llamacpp::LlamaCppProvider)> = None;
        let local_backend = std::env::var("LAPCE_AI_BACKEND").unwrap_or_else(|_| "candle".into());
        ensure!(["candle", "llama-cpp", "mistral-rs"].contains(&local_backend.as_str()), "Unknown local AI backend");
        ensure!(local_backend != "llama-cpp" || cfg!(feature = "llama-cpp"), "llama-cpp backend was not compiled");
        let mut mistral_provider = if local_backend == "mistral-rs" {
            Some(mistralrs::MistralRsProvider::new(PathBuf::from(std::env::var("LAPCE_AI_MISTRAL_WORKER")
                .map_err(|_| anyhow!("Set LAPCE_AI_MISTRAL_WORKER to the explicit worker executable"))?))?)
        } else { None };
        for request in rx {
            if let Some(remote) = request.remote.clone() {
                // Network lane: no local model files are touched. A remote
                // failure exits the worker like a local one; the dispatcher
                // respawns it. Identical configurations reuse the HTTP pool.
                let id = request.request_id;
                let cancelled = {
                    let cancel_for = cancel_for.clone();
                    move || {
                        let named = cancel_for.load(Ordering::Acquire);
                        named == id || (id == 0 && named != u64::MAX)
                    }
                };
                let start = Instant::now();
                let output = match ollama::cached_provider(&mut remote_provider, remote)
                    .complete(&request, &cancelled)
                {
                    Ok(output) => output,
                    Err(e) if is_cancelled(&e) => Completion {
                        text: String::new(),
                        tokens: 0,
                        prompt_tokens: 0,
                        load_ms: 0,
                        generation_ms: start.elapsed().as_millis(),
                        prefill_ms: 0,
                        decode_ms: 0,
                        cancelled: true,
                    },
                    Err(e) => return Err(e),
                };
                println!("{}", serde_json::to_string(&output)?);
                std::io::stdout().flush()?;
                continue;
            }
            if let Some(provider) = mistral_provider.as_mut() {
                let id = request.request_id;
                let cancelled = || {
                    let named = cancel_for.load(Ordering::Acquire);
                    named == id || (id == 0 && named != u64::MAX)
                };
                let start = Instant::now();
                let output = match provider.complete(&request, &cancelled) {
                    Ok(output) => output,
                    Err(error) if is_cancelled(&error) => Completion {
                        text: String::new(), tokens: 0, prompt_tokens: 0, load_ms: 0,
                        generation_ms: start.elapsed().as_millis(), prefill_ms: 0, decode_ms: 0, cancelled: true,
                    },
                    Err(error) => return Err(error),
                };
                println!("{}", serde_json::to_string(&output)?);
                std::io::stdout().flush()?;
                continue;
            }
            #[cfg(feature = "llama-cpp")]
            if local_backend == "llama-cpp" {
                let id = request.request_id;
                let cancelled = || {
                    let named = cancel_for.load(Ordering::Acquire);
                    named == id || (id == 0 && named != u64::MAX)
                };
                let start = Instant::now();
                let result = (|| -> Result<Completion> {
                    ensure!(!cancelled(), "Cancelled");
                    let replace = llama_provider.as_ref().is_none_or(|(path, _)| *path != request.model);
                    replace_loaded(&mut llama_provider, replace, &cancelled,
                        || Ok((request.model.clone(), llamacpp::LlamaCppProvider::load(&request.model)?)))?;
                    llama_provider.as_mut().unwrap().1.complete(&request, &cancelled)
                })();
                let output = match result {
                    Ok(output) => output,
                    Err(error) if is_cancelled(&error) => Completion {
                        text: String::new(), tokens: 0, prompt_tokens: 0, load_ms: 0,
                        generation_ms: start.elapsed().as_millis(), prefill_ms: 0, decode_ms: 0, cancelled: true,
                    },
                    Err(error) => return Err(error),
                };
                println!("{}", serde_json::to_string(&output)?);
                std::io::stdout().flush()?;
                continue;
            }
            let id = request.request_id;
            let cancelled = {
                let cancel_for = cancel_for.clone();
                move || {
                    let named = cancel_for.load(Ordering::Acquire);
                    named == id || (id == 0 && named != u64::MAX)
                }
            };
            let start = Instant::now();
            let result = (|| -> Result<Completion> {
                let replace = provider.as_ref().is_none_or(|(m, t, _)| *m != request.model || *t != request.tokenizer);
                replace_loaded(&mut provider, replace, &cancelled, || Ok((
                    request.model.clone(), request.tokenizer.clone(),
                    CandleProvider::load_with_device(&request.model, &request.tokenizer, selected_device()?)?,
                )))?;
                provider.as_mut().unwrap().2.complete(&request, &cancelled)
            })();
            let output =
                match result {
                    Ok(output) => output,
                    Err(e) if is_cancelled(&e) => Completion {
                        text: String::new(),
                        tokens: 0,
                        prompt_tokens: 0,
                        load_ms: provider.as_ref().map_or(0, |p| p.2.load_ms),
                        generation_ms: start.elapsed().as_millis(),
                        prefill_ms: 0,
                        decode_ms: 0,
                        cancelled: true,
                    },
                    Err(e) => return Err(e),
                };
            println!("{}", serde_json::to_string(&output)?);
            std::io::stdout().flush()?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(e) => {
            event("error", 0, 0, &e.to_string());
            eprintln!("AI worker: {e}");
            1
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn replacement_drops_old_weights_before_load_and_checks_cancellation() {
        use std::{cell::Cell, rc::Rc};
        struct Weights(Rc<Cell<usize>>);
        impl Drop for Weights { fn drop(&mut self) { self.0.set(self.0.get() - 1); } }
        let alive = Rc::new(Cell::new(1));
        let mut slot = Some(Weights(alive.clone()));
        super::replace_loaded(&mut slot, true, &|| false, || {
            assert_eq!(alive.get(), 0);
            alive.set(1); Ok(Weights(alive.clone()))
        }).unwrap();
        assert!(super::replace_loaded(&mut slot, true, &|| true, || -> anyhow::Result<Weights> { panic!("cancelled load must not run") }).is_err());
        assert_eq!(alive.get(), 1);
        assert!(super::replace_loaded(&mut slot, true, &|| false, || anyhow::bail!("fixture load failure")).is_err());
        assert!(slot.is_none()); assert_eq!(alive.get(), 0);
    }

    use super::*;
    #[test]
    fn vocabulary_checks_ids_and_token_mapping_not_only_size() {
        let vocab = std::collections::HashMap::from([("a".to_string(), 0), ("b".to_string(), 1)]);
        let correct = vec![gguf_file::Value::String("a".into()), gguf_file::Value::String("b".into())];
        assert!(validate_vocabulary(&vocab, 2, Some(&correct)).is_ok());
        assert!(validate_vocabulary(&vocab, 1, None).is_err());
        let swapped = vec![gguf_file::Value::String("b".into()), gguf_file::Value::String("a".into())];
        assert!(validate_vocabulary(&vocab, 2, Some(&swapped)).is_err());
        assert!(validate_vocabulary(&vocab, 2, Some(&vec![])).is_err());
    }

    #[test]
    fn suffix_overlap() {
        assert_eq!(clip_suffix("a + b\n}\nfn main() {}", "\n}"), "a + b");
        assert_eq!(clip_suffix("return $a + $b;", "\n}\n"), "return $a + $b;");
    }
    #[test]
    fn checksum_rejects_corruption() {
        let path = std::env::temp_dir()
            .join(format!("lapce-ai-hash-{}", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        assert!(
            verify(
                &path,
                3,
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            )
            .is_ok()
        );
        assert!(verify(&path, 4, "bad").is_err());
        assert!(verify(&path, 3, "bad").is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn greedy_ties_and_invalid() {
        assert_eq!(greedy(&[1., 2., 2., f32::NAN]).unwrap(), 1);
        assert!(greedy(&[f32::NAN]).is_err());
    }
    #[test]
    fn stops_and_limit() {
        let mut n = 0;
        assert_eq!(
            sample_loop(8, &[3], &|| false, |_| {
                n += 1;
                Ok(n)
            })
            .unwrap(),
            vec![1, 2]
        );
        assert_eq!(
            sample_loop(2, &[], &|| false, |_| Ok(4)).unwrap(),
            vec![4, 4]
        );
    }
    #[test]
    fn wire_parsing() {
        let request = Request {
            model: PathBuf::from("m.gguf"),
            tokenizer: PathBuf::from("t.json"),
            prefix: "fn a() {".into(),
            suffix: "}".into(),
            prefix_tokens: 4,
            suffix_tokens: 2,
            max_tokens: 8,
            request_id: 7,
            messages: None,
            remote: None,
        };
        let line = serde_json::to_string(&request).unwrap();
        match parse_wire(&line).unwrap() {
            Wire::Request(r) => assert_eq!(r.request_id, 7),
            other => panic!("expected request, got {other:?}"),
        }
        match parse_wire(&cancel_line(7)).unwrap() {
            Wire::Cancel(7) => {}
            other => panic!("expected cancel 7, got {other:?}"),
        }
        assert_eq!(cancel_line(7), "{\"cancel_request\":7}");
        assert!(parse_wire("not json").is_err());
    }
    #[test]
    fn cancellation() {
        assert!(sample_loop(8, &[], &|| true, |_| panic!("must not run")).is_err());
    }
    #[test]
    fn prompt_and_unicode() {
        let path = std::env::var("AI_TOKENIZER").unwrap_or_default();
        if path.is_empty() {
            return;
        }
        let t = Tokenizer::from_file(path).unwrap();
        let r = Request {
            model: PathBuf::new(),
            tokenizer: PathBuf::new(),
            prefix: "fn café() {".into(),
            suffix: "}".into(),
            prefix_tokens: 5,
            suffix_tokens: 2,
            max_tokens: 8,
            request_id: 0,
            messages: None,
            remote: None,
        };
        let p = prompt_ids(&t, &r).unwrap();
        assert_eq!(p[0], token(&t, "fim_prefix").unwrap());
        assert_eq!(*p.last().unwrap(), token(&t, "fim_middle").unwrap());
        assert!(p.len() <= 10);
    }
    #[test]
    #[ignore = "requires real model via AI_MODEL and AI_TOKENIZER"]
    fn real_fim() {
        let m = PathBuf::from(std::env::var("AI_MODEL").unwrap());
        let t = PathBuf::from(std::env::var("AI_TOKENIZER").unwrap());
        let mut provider = CandleProvider::load(&m, &t).unwrap();
        for (prefix, suffix) in [
            ("fn add(a: i32, b: i32) -> i32 {\n    ", "\n}"),
            ("<?php\nfunction add($a, $b) {\n    ", "\n}\n"),
        ] {
            let r = Request {
                model: m.clone(),
                tokenizer: t.clone(),
                prefix: prefix.into(),
                suffix: suffix.into(),
                prefix_tokens: 128,
                suffix_tokens: 64,
                max_tokens: 24,
                request_id: 0,
                messages: None,
                remote: None,
            };
            let c = provider.complete(&r, &|| false).unwrap();
            println!("{c:?}");
            assert!(!c.text.trim().is_empty());
            assert!(c.text.contains('a') && c.text.contains('b'));
            assert!(!c.text.contains("```"));
        }
    }

    #[test]
    fn request_json_without_remote_stays_local() {
        // Older editors send no remote field: the worker must keep treating
        // those requests as local Candle work.
        let line = "{\"model\":\"m.gguf\",\"tokenizer\":\"t.json\",\"prefix\":\"a\",\"suffix\":\"b\",\"prefix_tokens\":4,\"suffix_tokens\":2,\"max_tokens\":8,\"request_id\":3,\"messages\":null}";
        let request: Request = serde_json::from_str(line).unwrap();
        assert!(request.remote.is_none());
        assert_eq!(request.request_id, 3);
        let with_remote = Request {
            remote: Some(ollama::RemoteFim {
                endpoint: "https://ai.izdrail.com".into(),
                model: "qwen2.5-coder:1.5b-instruct-q4_K_M".into(),
                timeout_ms: 15_000,
            }),
            ..request
        };
        let round: Request =
            serde_json::from_str(&serde_json::to_string(&with_remote).unwrap())
                .unwrap();
        let expected = Some("https://ai.izdrail.com");
        assert_eq!(round.remote.as_ref().map(|r| r.endpoint.as_str()), expected.as_deref());
    }
}
