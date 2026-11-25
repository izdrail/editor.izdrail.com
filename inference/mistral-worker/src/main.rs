//! Isolated offline Mistral.rs worker, preserving the editor's JSON request shape.
//! Candidate implementation. Exact-model/CUDA execution remains unverified.
use anyhow::{Result, anyhow, ensure};
use mistralrs::{GgufModelBuilder, Model, RequestBuilder, TextMessageRole};
use serde::Deserialize;
use std::{io::{BufRead, Write}, path::PathBuf, time::Instant};

#[derive(Deserialize)]
struct Message { role: String, content: String }
#[derive(Deserialize)]
struct Request {
    model: PathBuf,
    messages: Option<Vec<Message>>,
    max_tokens: usize,
    #[serde(default)]
    remote: Option<serde_json::Value>,
}

async fn load(path: &PathBuf) -> Result<Model> {
    ensure!(path.is_file(), "Local GGUF model file is missing");
    // Canonical local directory, never a Hub repository identifier.
    let path = path.canonicalize()?;
    let directory = path.parent().ok_or_else(|| anyhow!("Model directory missing"))?;
    let filename = path.file_name().and_then(|s| s.to_str()).ok_or_else(|| anyhow!("Model filename must be UTF-8"))?;
    ensure!(std::env::var("HF_HUB_OFFLINE").as_deref() == Ok("1"), "HF_HUB_OFFLINE=1 is required; this worker never downloads");
    let mut builder = GgufModelBuilder::new(directory.to_string_lossy(), vec![filename])
        .with_max_model_len(4096).with_max_num_seqs(1);
    let device = std::env::var("LAPCE_AI_DEVICE").unwrap_or_else(|_| "cpu".into());
    ensure!(["cpu", "cuda"].contains(&device.as_str()), "Mistral worker device must be cpu or cuda");
    if device == "cpu" { builder = builder.with_force_cpu(); }
    else { ensure!(cfg!(feature = "cuda"), "CUDA requested but not compiled"); }
    eprintln!("AI backend=mistral.rs requested_device={device}; verify actual runtime placement in logs and telemetry");
    builder.build().await
}

async fn complete(model: &Model, request: Request, load_ms: u128) -> Result<serde_json::Value> {
    ensure!(request.remote.is_none(), "Mistral worker is local only");
    ensure!((1..=512).contains(&request.max_tokens), "Chat output limit must be 1..512");
    let messages = request.messages.ok_or_else(|| anyhow!("Mistral worker raw FIM is not implemented; use a supported FIM backend"))?;
    ensure!(!messages.is_empty(), "Empty conversation");
    let mut prompt = RequestBuilder::new().set_deterministic_sampler().set_sampler_max_len(request.max_tokens);
    for message in messages {
        let role = match message.role.as_str() {
            "system" => TextMessageRole::System,
            "user" => TextMessageRole::User,
            "assistant" => TextMessageRole::Assistant,
            _ => return Err(anyhow!("Unsupported chat role")),
        };
        prompt = prompt.add_message(role, message.content.replace("<|", "< | "));
    }
    let started = Instant::now();
    let response = model.send_chat_request(prompt).await?;
    let text = response.choices.first().and_then(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("Mistral worker received no completion text"))?;
    Ok(serde_json::json!({
        "text": text, "tokens":response.usage.completion_tokens,
        "prompt_tokens":response.usage.prompt_tokens,
        "load_ms":load_ms, "generation_ms":started.elapsed().as_millis(),
        "prefill_ms":(response.usage.total_prompt_time_sec as f64 * 1000.0) as u128,
        "decode_ms":(response.usage.total_completion_time_sec as f64 * 1000.0) as u128,
        "cancelled":false
    }))
}

#[tokio::main]
async fn main() -> Result<()> {
    ensure!(std::env::var("HF_HUB_OFFLINE").as_deref() == Ok("1"), "Set HF_HUB_OFFLINE=1 for explicit offline operation");
    let mut cached: Option<(PathBuf, Model, u128)> = None;
    // The outer editor adapter owns hard cancellation by killing this process.
    // Do not pretend a dropped SDK future guarantees stopped GPU execution.
    let mut input = std::io::stdin().lock();
    while let Some(line) = read_frame(&mut input)? {
        let value: serde_json::Value = serde_json::from_str(&line)?;
        ensure!(value.get("cancel_request").is_none(), "Cooperative SDK cancellation is not implemented; use hard process cancellation");
        let request: Request = serde_json::from_value(value)?;
        ensure!(request.remote.is_none(), "Remote execution not allowed");
        if cached.as_ref().is_none_or(|(path,_,_)| *path != request.model) {
            // Release old weights before loading replacements to avoid a
            // transient two-model allocation peak in the isolated worker.
            cached = None;
            let started = Instant::now();
            let model = load(&request.model).await?;
            cached = Some((request.model.clone(),model,started.elapsed().as_millis()));
        }
        let (_,model,load_ms) = cached.as_ref().unwrap();
        let result = complete(model,request,*load_ms).await?;
        println!("{result}");
        std::io::stdout().flush()?;
    }
    Ok(())
}

// Bound allocation before accepting a complete JSON-lines request.
fn read_frame(reader: &mut impl BufRead) -> Result<Option<String>> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            ensure!(line.is_empty(), "Incomplete Mistral request frame");
            return Ok(None);
        }
        let end = available.iter().position(|&b| b == b'\n');
        let count = end.map_or(available.len(), |n| n + 1);
        ensure!(line.len().checked_add(count).is_some_and(|n| n <= 8 * 1024 * 1024), "Mistral request exceeds 8 MiB frame limit");
        line.extend_from_slice(&available[..count]);
        reader.consume(count);
        if end.is_some() { return Ok(Some(String::from_utf8(line)?)); }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_rejects_partial_invalid_and_oversized_input() {
        for bytes in [b"{}".to_vec(), vec![255, 10], vec![b'x'; 8 * 1024 * 1024 + 1]] {
            assert!(read_frame(&mut std::io::Cursor::new(bytes)).is_err());
        }
        let mut source = std::io::Cursor::new(b"{}\n[]\n");
        assert_eq!(read_frame(&mut source).unwrap().unwrap(), "{}\n");
        assert_eq!(read_frame(&mut source).unwrap().unwrap(), "[]\n");
        assert!(read_frame(&mut source).unwrap().is_none());
    }
}
