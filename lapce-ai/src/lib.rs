//! Local CPU-only Qwen FIM provider. No implicit downloads or remote inference.
pub mod process;
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
impl CandleProvider {
    pub fn load(model: &Path, tokenizer: &Path) -> Result<Self> {
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
        let tokenizer =
            Tokenizer::from_file(tokenizer).map_err(|e| anyhow!(e.to_string()))?;
        for name in ["fim_prefix", "fim_suffix", "fim_middle", "endoftext"] {
            token(&tokenizer, name)?;
        }
        let model = ModelWeights::from_gguf(content, &mut reader, &Device::Cpu)?;
        Ok(Self {
            model,
            tokenizer,
            load_ms: start.elapsed().as_millis(),
        })
    }
}
impl Provider for CandleProvider {
    fn complete(
        &mut self,
        request: &Request,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Completion> {
        let ids = prompt_ids(&self.tokenizer, request)?;
        let stops = [
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
            let input = Tensor::new(input, &Device::Cpu)?.unsqueeze(0)?;
            let logits = self
                .model
                .forward(&input, pos)?
                .squeeze(0)?
                .to_vec1::<f32>()?;
            if out.is_empty() {
                prefill_ms = step.elapsed().as_millis();
            }
            greedy(&logits)
        })?;
        let text = self
            .tokenizer
            .decode(&generated, true)
            .map_err(|e| anyhow!(e.to_string()))?;
        let generation_ms = start.elapsed().as_millis();
        Ok(Completion {
            text: clip_suffix(&text, &request.suffix),
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
        "QuantFactory/Qwen2.5-Coder-1.5B-GGUF",
        MODEL_REV,
        MODEL_FILE,
        986048352,
        MODEL_HASH,
    )?;
    let tokenizer = obtain_progress(
        "Qwen/Qwen2.5-Coder-1.5B",
        TOKEN_REV,
        "tokenizer.json",
        7031645,
        TOKEN_HASH,
    )?;
    let installed = Installed { model, tokenizer };
    let temp = root.join("installed.json.part");
    std::fs::write(&temp, serde_json::to_vec_pretty(&installed)?)?;
    std::fs::rename(temp, root.join("installed.json"))?;
    event("ready", 0, 0, "Qwen2.5-Coder 1.5B base Q4_K_M installed");
    Ok(installed)
}
/// Cooperative cancellation needs a control thread: stdin carries both
/// requests and cancellation lines, but inference blocks the serving thread,
/// so it could never read a cancellation from the same stream in time.
pub fn run() -> i32 {
    let result = (|| -> Result<()> {
        if std::env::args().nth(1).as_deref() == Some("--ai-download") {
            let root = std::env::args()
                .nth(2)
                .ok_or_else(|| anyhow!("Missing download directory"))?;
            download(Path::new(&root))?;
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
        for request in rx {
            if provider.as_ref().is_none_or(|(m, t, _)| {
                *m != request.model || *t != request.tokenizer
            }) {
                provider = Some((
                    request.model.clone(),
                    request.tokenizer.clone(),
                    CandleProvider::load(&request.model, &request.tokenizer)?,
                ));
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
            let output =
                match provider.as_mut().unwrap().2.complete(&request, &cancelled) {
                    Ok(output) => output,
                    Err(e) if is_cancelled(&e) => Completion {
                        text: String::new(),
                        tokens: 0,
                        prompt_tokens: 0,
                        load_ms: provider.as_ref().unwrap().2.load_ms,
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
    use super::*;
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
            };
            let c = provider.complete(&r, &|| false).unwrap();
            println!("{c:?}");
            assert!(!c.text.trim().is_empty());
            assert!(c.text.contains('a') && c.text.contains('b'));
            assert!(!c.text.contains("```"));
        }
    }
}
