//! Killable, reusable worker process. Dispatcher only queues work, never evaluates tensors.
//!
//! Cancellation has two levels:
//! - Soft: a cancellation line names the in-flight request id. The worker
//!   stops between tokens, answers the cancelled request with an empty
//!   completion so the pipe stays in sync, and keeps the loaded model warm.
//! - Hard: the process is killed. Used on shutdown or when the soft channel
//!   is broken; the next request pays a full model reload.
//!
//! Rate discipline: at most one request is in flight and at most one is
//! pending. A newer request supersedes a pending one ("latest wins"), so
//! automatic triggers cannot pile up behind slow CPU inference.
use crate::{Completion, Request, cancel_line};
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
type Reply = Box<dyn FnOnce(Result<Completion, String>) + Send>;
struct Job {
    generation: u64,
    request: Request,
    reply: Reply,
}
struct Shared {
    child: Mutex<Option<Child>>,
    /// Held only for individual writes so soft cancellation can interrupt a
    /// blocking read without waiting for the serving thread.
    stdin: Mutex<Option<ChildStdin>>,
    pending: Mutex<VecDeque<Job>>,
    ready: Condvar,
    generation: AtomicU64,
    busy: AtomicBool,
}
#[derive(Clone)]
pub struct Worker {
    shared: Arc<Shared>,
}
impl Worker {
    pub fn new(executable: PathBuf) -> Self {
        let shared = Arc::new(Shared {
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            pending: Mutex::new(VecDeque::with_capacity(1)),
            ready: Condvar::new(),
            generation: AtomicU64::new(0),
            busy: AtomicBool::new(false),
        });
        std::thread::spawn({
            let shared = shared.clone();
            move || serve(executable, shared)
        });
        Self { shared }
    }
    /// Hard cancellation: kill the worker process. The next request reloads
    /// the model. Used on shutdown or when the soft channel failed.
    pub fn cancel(&self) {
        self.shared.generation.fetch_add(1, Ordering::AcqRel);
        self.shared.busy.store(false, Ordering::Release);
        if let Some(job) = self.shared.pending.lock().unwrap().pop_front() {
            (job.reply)(Err("Cancelled".into()));
        }
        kill_child(&self.shared);
    }
    /// Cooperative cancellation of the in-flight request, keeping the loaded
    /// model warm. Falls back to a hard kill when the worker is unreachable.
    pub fn soft_cancel(&self) {
        let was_busy = self.shared.busy.swap(false, Ordering::AcqRel);
        let in_flight = self.shared.generation.fetch_add(1, Ordering::AcqRel);
        if !was_busy || in_flight == 0 {
            return;
        }
        let sent = self
            .shared
            .stdin
            .lock()
            .unwrap()
            .as_mut()
            .map(|stdin| {
                writeln!(stdin, "{}", cancel_line(in_flight))
                    .and_then(|()| stdin.flush())
                    .is_ok()
            })
            .unwrap_or(false);
        if !sent {
            // The pipe is gone; the serving thread may block on a dead child
            // forever. Kill so the next request respawns cleanly.
            kill_child(&self.shared);
        }
    }
    pub fn complete(
        &self,
        mut request: Request,
        reply: impl FnOnce(Result<Completion, String>) + Send + 'static,
    ) {
        if self.shared.busy.load(Ordering::Acquire) {
            // New keystrokes cancel the in-flight request before replacing it.
            self.soft_cancel();
        }
        let generation = self.shared.generation.fetch_add(1, Ordering::AcqRel) + 1;
        request.request_id = generation;
        self.shared.busy.store(true, Ordering::Release);
        let job = Job {
            generation,
            request,
            reply: Box::new(reply),
        };
        let mut pending = self.shared.pending.lock().unwrap();
        if let Some(stale) = pending.pop_front() {
            // Bounded queue: never let automatic requests pile up.
            (stale.reply)(Err("Superseded by a newer request".into()));
        }
        pending.push_back(job);
        drop(pending);
        self.shared.ready.notify_one();
    }
}
fn kill_child(shared: &Shared) {
    if let Some(mut child) = shared.child.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    *shared.stdin.lock().unwrap() = None;
}
fn serve(executable: PathBuf, shared: Arc<Shared>) {
    let mut stdout: Option<BufReader<ChildStdout>> = None;
    loop {
        let job = {
            let mut pending = shared.pending.lock().unwrap();
            loop {
                if let Some(job) = pending.pop_front() {
                    break job;
                }
                pending = shared.ready.wait(pending).unwrap();
            }
        };
        if shared.generation.load(Ordering::Acquire) != job.generation {
            shared.busy.store(false, Ordering::Release);
            (job.reply)(Err("Cancelled".into()));
            continue;
        }
        let result = (|| -> anyhow::Result<Completion> {
            let dead = {
                let mut slot = shared.child.lock().unwrap();
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
                let stdin = process.stdin.take().unwrap();
                stdout = Some(BufReader::new(process.stdout.take().unwrap()));
                let mut slot = shared.child.lock().unwrap();
                if shared.generation.load(Ordering::Acquire) != job.generation {
                    let _ = process.kill();
                    let _ = process.wait();
                    anyhow::bail!("Cancelled");
                }
                *shared.stdin.lock().unwrap() = Some(stdin);
                *slot = Some(process);
            }
            {
                let mut slot = shared.stdin.lock().unwrap();
                let Some(stdin) = slot.as_mut() else {
                    anyhow::bail!("Worker stdin closed");
                };
                writeln!(stdin, "{}", serde_json::to_string(&job.request)?)?;
                stdin.flush()?;
            }
            let mut line = String::new();
            let Some(output) = stdout.as_mut() else {
                anyhow::bail!("Worker stdout closed");
            };
            output.read_line(&mut line)?;
            if line.is_empty() {
                anyhow::bail!("Worker exited without answering");
            }
            Ok(serde_json::from_str(&line)?)
        })();
        let mut result = result.map_err(|e| e.to_string());
        // A dead worker must not be reused; drop it so the next job respawns.
        if result.is_err() {
            kill_child(&shared);
        }
        if shared.generation.load(Ordering::Acquire) != job.generation {
            result = Err("Cancelled".into());
        } else if matches!(&result, Ok(c) if c.cancelled) {
            result = Err("Cancelled".into());
        }
        shared.busy.store(false, Ordering::Release);
        (job.reply)(result);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn script(name: &str, body: &str) -> PathBuf {
        let path = std::env::temp_dir()
            .join(format!("lapce-ai-{name}-{}.sh", std::process::id()));
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .unwrap();
        path
    }
    fn request() -> Request {
        Request {
            model: PathBuf::new(),
            tokenizer: PathBuf::new(),
            prefix: String::new(),
            suffix: String::new(),
            prefix_tokens: 1,
            suffix_tokens: 0,
            max_tokens: 1,
            request_id: 0,
        }
    }
    const OK: &str = "{\"text\":\"ok$$\",\"tokens\":1,\"prompt_tokens\":1,\"load_ms\":0,\"generation_ms\":0,\"prefill_ms\":0,\"decode_ms\":0,\"cancelled\":false}";
    #[test]
    fn worker_reply_and_hard_cancel() {
        let script = script(
            "worker",
            "#!/bin/sh\nwhile read line; do\n sleep 1\n printf '%s\\n' '{\"text\":\"ok\",\"tokens\":1,\"prompt_tokens\":1,\"load_ms\":0,\"generation_ms\":0,\"prefill_ms\":0,\"decode_ms\":0}'\ndone\n",
        );
        let worker = Worker::new(script.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        worker.complete(request(), move |r| {
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
        worker.complete(request(), move |r| {
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
    #[test]
    fn soft_cancel_keeps_process_warm() {
        // Emulates the child protocol: requests get a pid-tagged answer after
        // a delay, cancellation lines get no answer of their own.
        let body = format!(
            "#!/bin/sh\nwhile read line; do\n case \"$line\" in\n  *cancel_request*) : ;;\n  *) sleep 1\n printf '%s\\n' '{OK}' ;;\n esac\ndone\n"
        );
        let script = script("soft", &body);
        let worker = Worker::new(script.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        worker.complete(request(), move |r| {
            let _ = tx.send(r);
        });
        let first = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .unwrap()
            .text;
        assert!(first.starts_with("ok"), "{first}");
        // Start a second request, then soft-cancel it while it runs.
        let (tx, rx) = std::sync::mpsc::channel();
        worker.complete(request(), move |r| {
            let _ = tx.send(r);
        });
        std::thread::sleep(std::time::Duration::from_millis(100));
        worker.soft_cancel();
        assert!(
            rx.recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .is_err()
        );
        // The next request must reuse the same warm process.
        let (tx, rx) = std::sync::mpsc::channel();
        worker.complete(request(), move |r| {
            let _ = tx.send(r);
        });
        let second = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .unwrap()
            .text;
        assert_eq!(first, second, "soft cancel must not kill the worker");
        worker.cancel();
        std::fs::remove_file(script).unwrap();
    }
    #[test]
    fn latest_wins_replaces_pending() {
        let body = format!(
            "#!/bin/sh\nwhile read line; do\n case \"$line\" in\n  *cancel_request*) : ;;\n  *) sleep 1\n printf '%s\\n' '{OK}' ;;\n esac\ndone\n"
        );
        let script = script("latest", &body);
        let worker = Worker::new(script.clone());
        let (tx1, rx1) = std::sync::mpsc::channel();
        worker.complete(request(), move |r| {
            let _ = tx1.send(r.map(|c| c.text));
        });
        // Occupy the worker, then queue two more requests. The second must
        // supersede the first pending one instead of piling up.
        let (tx2, rx2) = std::sync::mpsc::channel();
        worker.complete(request(), move |r| {
            let _ = tx2.send(r.map(|c| c.text));
        });
        let (tx3, rx3) = std::sync::mpsc::channel();
        worker.complete(request(), move |r| {
            let _ = tx3.send(r.map(|c| c.text));
        });
        // The in-flight job was cancelled by the newer request: latest wins.
        assert!(
            rx1.recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .is_err()
        );
        let superseded =
            rx2.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert!(superseded.is_err(), "pending job must be superseded");
        assert!(
            rx3.recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .is_ok()
        );
        worker.cancel();
        std::fs::remove_file(script).unwrap();
    }
}
