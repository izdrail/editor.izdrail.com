//! Killable, reusable worker process. Dispatcher only queues work, never evaluates tensors.
use crate::{Completion, Request};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
type Reply = Box<dyn FnOnce(Result<Completion, String>) + Send>;
struct Job {
    generation: u64,
    request: Request,
    reply: Reply,
}
#[derive(Clone)]
pub struct Worker {
    tx: std::sync::mpsc::SyncSender<Job>,
    child: Arc<Mutex<Option<Child>>>,
    generation: Arc<AtomicU64>,
    busy: Arc<AtomicBool>,
}
impl Worker {
    pub fn new(executable: PathBuf) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Job>(1);
        let child = Arc::new(Mutex::new(None::<Child>));
        let generation = Arc::new(AtomicU64::new(0));
        let busy = Arc::new(AtomicBool::new(false));
        let state = child.clone();
        let epoch = generation.clone();
        let active = busy.clone();
        std::thread::spawn(move || {
            let mut pipes = None;
            for job in rx {
                if epoch.load(Ordering::Acquire) != job.generation {
                    (job.reply)(Err("Cancelled".into()));
                    continue;
                }
                let result = (|| -> anyhow::Result<Completion> {
                    let dead = {
                        let mut slot = state.lock().unwrap();
                        match slot.as_mut() {
                            None => true,
                            Some(c) => c.try_wait()?.is_some(),
                        }
                    };
                    if dead {
                        let mut process = Command::new(&executable)
                            .arg("--ai-worker")
                            .stdin(Stdio::piped())
                            .stdout(Stdio::piped())
                            .stderr(Stdio::null())
                            .spawn()?;
                        pipes = Some((
                            process.stdin.take().unwrap(),
                            BufReader::new(process.stdout.take().unwrap()),
                        ));
                        let mut slot = state.lock().unwrap();
                        if epoch.load(Ordering::Acquire) != job.generation {
                            let _ = process.kill();
                            let _ = process.wait();
                            anyhow::bail!("Cancelled");
                        }
                        *slot = Some(process);
                    }
                    let (input, output) = pipes.as_mut().unwrap();
                    writeln!(input, "{}", serde_json::to_string(&job.request)?)?;
                    input.flush()?;
                    let mut line = String::new();
                    output.read_line(&mut line)?;
                    let completion = serde_json::from_str(&line)?;
                    Ok(completion)
                })();
                if epoch.load(Ordering::Acquire) == job.generation {
                    active.store(false, Ordering::Release);
                    (job.reply)(result.map_err(|e| e.to_string()));
                } else {
                    (job.reply)(Err("Cancelled".into()));
                }
            }
            if let Some(mut c) = state.lock().unwrap().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        });
        Self {
            tx,
            child,
            generation,
            busy,
        }
    }
    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.busy.store(false, Ordering::Release);
        if let Some(c) = self.child.lock().unwrap().as_mut() {
            let _ = c.kill();
        }
    }
    pub fn complete(
        &self,
        request: Request,
        reply: impl FnOnce(Result<Completion, String>) + Send + 'static,
    ) {
        if self.busy.load(Ordering::Acquire) {
            self.cancel();
        }
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.busy.store(true, Ordering::Release);
        if let Err(err) = self.tx.try_send(Job {
            generation,
            request,
            reply: Box::new(reply),
        }) {
            self.busy.store(false, Ordering::Release);
            let job = match err {
                std::sync::mpsc::TrySendError::Full(job)
                | std::sync::mpsc::TrySendError::Disconnected(job) => job,
            };
            (job.reply)(Err("AI queue full; retry manual invocation".into()));
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn worker_reply_and_hard_cancel() {
        let script = std::env::temp_dir()
            .join(format!("lapce-ai-worker-{}.sh", std::process::id()));
        std::fs::write(&script, b"#!/bin/sh\nwhile read line; do\n sleep 1\n printf '%s\\n' '{\"text\":\"ok\",\"tokens\":1,\"prompt_tokens\":1,\"load_ms\":0,\"generation_ms\":0,\"prefill_ms\":0,\"decode_ms\":0}'\ndone\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700))
            .unwrap();
        let worker = Worker::new(script.clone());
        let request = Request {
            model: PathBuf::new(),
            tokenizer: PathBuf::new(),
            prefix: String::new(),
            suffix: String::new(),
            prefix_tokens: 1,
            suffix_tokens: 0,
            max_tokens: 1,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        worker.complete(request.clone(), move |r| {
            let _ = tx.send(r);
        });
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .unwrap()
                .text,
            "ok"
        );
        let (tx, rx) = std::sync::mpsc::channel();
        worker.complete(request, move |r| {
            let _ = tx.send(r);
        });
        std::thread::sleep(std::time::Duration::from_millis(100));
        worker.cancel();
        assert!(
            rx.recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .is_err()
        );
        drop(worker);
        std::fs::remove_file(script).unwrap();
    }
}
