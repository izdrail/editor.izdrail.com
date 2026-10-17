//! ACP agent subprocess plumbing: launch an agent (`opencode acp`, an
//! adapter binary) and exchange NDJSON JSON-RPC frames over its stdio.
//! This is transport only: it does not choose an agent, decide
//! capabilities, grant filesystem or terminal access, apply edits or
//! render UI. Callers must run it OFF the UI thread (blocking reads) and
//! own a watchdog for unresponsive agents; `recv` has no timeout of its
//! own. stderr is drained on a background thread into a small capped ring
//! so a chatty agent cannot deadlock on a full pipe.

use std::{
    io::{Read, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    },
    thread::JoinHandle,
};

use serde_json::Value;

use crate::agent_acp::{self, AcpError, AcpMessage};

const STDERR_LINES: usize = 100;
const MAX_LINE: usize = 1 << 20;

static NEXT_ID: AtomicI64 = AtomicI64::new(1);

/// Fresh JSON-RPC request ids for this process.
pub fn next_id() -> i64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub struct AcpProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: Arc<Mutex<Vec<String>>>,
    _drain: JoinHandle<()>,
}

impl AcpProcess {
    /// Launch `program` with `args` in `cwd`. No lookup, shell or env
    /// manipulation: the caller passes the exact binary.
    pub fn spawn(program: &str, args: &[&str], cwd: &std::path::Path) -> std::io::Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let ring: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let drain = {
            let ring = ring.clone();
            let mut stderr = child.stderr.take().expect("piped stderr");
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                let mut line = String::new();
                loop {
                    match stderr.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            line.push_str(&String::from_utf8_lossy(&buf[..n]));
                            while let Some(p) = line.find('\n') {
                                let l = line[..p].to_string();
                                line = line[p + 1..].to_string();
                                let mut g = ring.lock().unwrap();
                                if g.len() >= STDERR_LINES {
                                    g.remove(0);
                                }
                                g.push(l);
                            }
                        }
                    }
                }
            })
        };
        Ok(AcpProcess { child, stdin, stdout, stderr: ring, _drain: drain })
    }

    /// Recent stderr lines (agent logs), oldest first, capped.
    pub fn stderr_log(&self) -> Vec<String> {
        self.stderr.lock().unwrap().clone()
    }

    /// Send one frame.
    pub fn send(&mut self, message: &Value) -> Result<(), AcpError> {
        let wire = agent_acp::encode(message)?;
        self.stdin.write_all(wire.as_bytes()).and_then(|_| self.stdin.flush()).map_err(|e| {
            AcpError { code: -32603, message: format!("write to agent failed: {e}") }
        })
    }

    /// Read one frame. Blocking; `Ok(None)` means the agent closed stdout
    /// (usually exited). Overlong or invalid lines are errors, not skips.
    pub fn recv(&mut self) -> Result<Option<AcpMessage>, AcpError> {
        let mut line = String::new();
        let mut chunk = [0u8; 8192];
        loop {
            if line.len() > MAX_LINE {
                return Err(AcpError { code: -32603, message: "agent line over 1 MiB".into() });
            }
            let n = self.stdout.read(&mut chunk).map_err(|e| AcpError {
                code: -32603,
                message: format!("read from agent failed: {e}"),
            })?;
            if n == 0 {
                return Ok(None);
            }
            line.push_str(&String::from_utf8_lossy(&chunk[..n]));
            if line.ends_with('\n') {
                return agent_acp::decode(&line).map(Some);
            }
        }
    }

    /// Kill the child (best effort) and reap it.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for AcpProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_through_a_real_subprocess() {
        // `cat` echoes each NDJSON line back unchanged.
        let mut p = AcpProcess::spawn("cat", &[], std::path::Path::new("/tmp")).unwrap();
        let msg = agent_acp::session_prompt(next_id(), "s1", "hello acp");
        p.send(&msg).unwrap();
        match p.recv().unwrap() {
            Some(AcpMessage::Request { method, params, .. }) => {
                assert_eq!(method, "session/prompt");
                assert_eq!(params["sessionId"], serde_json::json!("s1"));
            }
            other => panic!("unexpected echo: {other:?}"),
        }
        p.kill();
    }

    #[test]
    fn eof_and_missing_binary() {
        // `true` exits immediately: stdout closes, recv reports EOF.
        let mut p = AcpProcess::spawn("true", &[], std::path::Path::new("/tmp")).unwrap();
        assert_eq!(p.recv().unwrap(), None);
        assert!(AcpProcess::spawn("not-a-real-acp-agent-xyz", &[], std::path::Path::new("/tmp")).is_err());
    }
}
