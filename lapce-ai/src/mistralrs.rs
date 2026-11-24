//! Local process boundary isolates Mistral.rs dependencies from the editor.
use anyhow::{Result, anyhow, bail, ensure};
use std::{io::{BufRead, BufReader, Write}, path::{Path, PathBuf}, process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError}, time::{Duration, Instant}};
use crate::{Completion, Provider, Request};

pub struct MistralRsProvider {
    executable: PathBuf,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    responses: Option<Receiver<std::result::Result<Completion, String>>>,
}
impl MistralRsProvider {
    pub fn new(executable: PathBuf) -> Result<Self> {
        ensure!(executable.is_absolute() && executable.is_file(), "Mistral worker must be an explicit existing absolute executable path");
        Ok(Self { executable, child: None, stdin: None, responses: None })
    }
    fn stop(&mut self) {
        self.stdin = None;
        if let Some(mut child) = self.child.take() { let _ = child.kill(); let _ = child.wait(); }
        self.responses = None;
    }
    fn start(&mut self) -> Result<()> {
        if self.child.as_mut().is_some_and(|c| c.try_wait().is_ok_and(|s| s.is_none())) { return Ok(()); }
        self.stop();
        let mut child = Command::new(&self.executable).env("HF_HUB_OFFLINE", "1")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn()?;
        self.stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("Mistral worker stdout unavailable"))?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                let result = match std::io::Read::take(&mut reader, 8 * 1024 * 1024 + 1).read_line(&mut line) {
                    Ok(0) => Err("Mistral worker closed stdout".into()),
                    Ok(n) if n > 8 * 1024 * 1024 => Err("Mistral completion exceeds limit".into()),
                    Ok(_) => serde_json::from_str::<Completion>(&line).map_err(|_| "Mistral worker returned invalid completion JSON".into()),
                    Err(_) => Err("Mistral worker response read failed".into()),
                };
                let failed = result.is_err();
                if tx.send(result).is_err() || failed { break; }
            }
        });
        self.responses = Some(rx);
        self.child = Some(child);
        Ok(())
    }
}
impl Drop for MistralRsProvider { fn drop(&mut self) { self.stop(); } }
impl Provider for MistralRsProvider {
    fn complete(&mut self, request: &Request, cancelled: &dyn Fn() -> bool) -> Result<Completion> {
        ensure!(request.remote.is_none(), "Mistral provider is local only");
        ensure!(request.messages.is_some(), "Mistral provider raw FIM is not implemented");
        ensure!(Path::new(&request.model).is_file(), "Local Mistral model is missing");
        ensure!(!cancelled(), "Cancelled");
        let body = serde_json::to_vec(request)?;
        ensure!(body.len() <= 8 * 1024 * 1024, "Mistral request exceeds limit");
        self.start()?;
        let result = (|| -> Result<Completion> {
            let mut stdin = self.stdin.take().ok_or_else(|| anyhow!("Mistral stdin unavailable"))?;
            let (written_tx, written_rx) = mpsc::channel();
            std::thread::spawn(move || {
                let written = (|| -> std::io::Result<()> {
                    stdin.write_all(&body)?; stdin.write_all(b"\n")?; stdin.flush()
                })();
                let _ = written_tx.send((stdin, written));
            });
            let started = Instant::now();
            loop {
                ensure!(!cancelled(), "Cancelled");
                ensure!(started.elapsed() < Duration::from_secs(300), "Mistral request write deadline exceeded");
                match written_rx.recv_timeout(Duration::from_millis(25)) {
                    Ok((stdin, result)) => { self.stdin = Some(stdin); result?; break; }
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(_) => bail!("Mistral request writer closed"),
                }
            }
            loop {
                if cancelled() { bail!("Cancelled"); }
                ensure!(started.elapsed() < Duration::from_secs(300), "Mistral worker response deadline exceeded");
                match self.responses.as_ref().unwrap().recv_timeout(Duration::from_millis(25)) {
                    Ok(Ok(output)) => return Ok(output),
                    Ok(Err(error)) => bail!("{error}"),
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(_) => bail!("Mistral worker response channel closed"),
                }
            }
        })();
        if result.is_err() { self.stop(); }
        result
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn cancellation_interrupts_a_blocked_request_write() {
        let root = std::env::temp_dir().join(format!("mistral-blocked-{}",std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let exe = root.join("worker.sh");
        std::fs::write(&exe,"#!/bin/sh\nexec sleep 10\n").unwrap();
        std::fs::set_permissions(&exe,std::fs::Permissions::from_mode(0o700)).unwrap();
        let model = root.join("model");std::fs::write(&model,b"fixture").unwrap();
        let request: Request = serde_json::from_value(serde_json::json!({"model":model,"tokenizer":"", "prefix":"", "suffix":"", "prefix_tokens":1,"suffix_tokens":0,"max_tokens":1,"messages":[{"role":"user","content":"x".repeat(1024*1024)}]})).unwrap();
        let mut provider = MistralRsProvider::new(exe).unwrap();
        let started = Instant::now();
        let result = provider.complete(&request,&|| started.elapsed()>Duration::from_millis(150));
        assert!(result.unwrap_err().to_string().contains("Cancelled"));
        assert!(started.elapsed()<Duration::from_secs(3));
        assert!(provider.child.is_none());
        drop(provider);std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn adapter_reads_compatible_completion_and_kills_on_cancel() {
        let root = std::env::temp_dir().join(format!("mistral-adapter-{}",std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let exe = root.join("worker.sh");
        std::fs::write(&exe,"#!/bin/sh\nwhile read line; do printf '%s\\n' '{\"text\":\"ok\",\"tokens\":1,\"prompt_tokens\":1,\"load_ms\":0,\"generation_ms\":1,\"prefill_ms\":0,\"decode_ms\":1}'; done\n").unwrap();
        std::fs::set_permissions(&exe,std::fs::Permissions::from_mode(0o700)).unwrap();
        let model = root.join("model");std::fs::write(&model,b"fixture").unwrap();
        let request: Request = serde_json::from_value(serde_json::json!({"model":model,"tokenizer":"", "prefix":"", "suffix":"", "prefix_tokens":1,"suffix_tokens":0,"max_tokens":1,"messages":[{"role":"user","content":"hi"}]})).unwrap();
        let mut provider = MistralRsProvider::new(exe.clone()).unwrap();
        assert_eq!(provider.complete(&request,&||false).unwrap().text,"ok");
        let calls = std::cell::Cell::new(0);
        assert!(provider.complete(&request,&||{calls.set(calls.get()+1);calls.get()>1}).is_err());
        assert!(provider.child.is_none());
        drop(provider);std::fs::remove_dir_all(root).unwrap();
    }
}
