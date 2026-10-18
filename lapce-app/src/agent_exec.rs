//! Owner-approved shell runner for Act mode's run_command tool.
//! Approval lives in the chat panel; this module never asks, never
//! persists trust, and never runs anything the owner has not just
//! approved for this exact command string.
use std::{
    path::Path,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Combined stdout+stderr cap fed back to the model.
const OUTPUT_CAP: usize = 64 * 1024;

/// Kill the whole process group (the shell plus any grandchildren
/// still holding the pipes), then reap the shell. Group-kill via the
/// `kill` binary: lapce-app has no libc dep. On non-unix only the
/// shell itself is killed. If the group is already gone this is a
/// no-op; the window for the pgid being recycled before this call is
/// microseconds and the worst case is a failed kill we ignore.
fn teardown(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let pgid = child.id();
        let _ = Command::new("kill")
            .args(["-KILL", &format!("-{pgid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Run `sh -c <command>` in `root`: stdin closed, combined output
/// capped, wall-clock limited, killed on cancel. Failing exit codes
/// are Err with the capped output attached.
pub fn run_shell_command(
    root: &Path,
    command: &str,
    timeout: Duration,
    cancelled: Arc<AtomicBool>,
) -> Result<String, String> {
    if command.trim().is_empty() || command.len() > 4096 {
        return Err("Empty or oversized command".into());
    }
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0");
    // Own process group: `sh -c` pipelines spawn grandchildren that
    // hold the output pipes; killing only the shell leaves them (and
    // our reader threads) stuck. pgid == child pid after this.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(|e| format!("Could not start sh: {e}"))?;
    // Drain both pipes on reader threads: a producer writing more
    // than the pipe buffer must never block until the timeout.
    let mut out_pipe = child.stdout.take().ok_or("Missing stdout pipe")?;
    let mut err_pipe = child.stderr.take().ok_or("Missing stderr pipe")?;
    let out_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut out_pipe, &mut buf);
        buf
    });
    let err_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut err_pipe, &mut buf);
        buf
    });
    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let mut status = None;
    loop {
        if cancelled.load(Ordering::Acquire) {
            teardown(&mut child);
            let _ = out_handle.join();
            let _ = err_handle.join();
            return Err("Cancelled".into());
        }
        match child.try_wait() {
            Ok(Some(s)) => {
                status = Some(s);
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("Could not wait for command: {e}")),
        }
    }
    // Kill the process group on every exit path: without it a
    // surviving grandchild holds the pipes and the reader joins below
    // would block past the wall-clock limit.
    teardown(&mut child);
    let stdout_bytes = out_handle.join().map_err(|_| "Output reader panicked")?;
    let stderr_bytes = err_handle.join().map_err(|_| "Output reader panicked")?;
    if timed_out {
        return Err(format!(
            "Command timed out after {}s",
            timeout.as_secs().max(1)
        ));
    }
    let status = status.ok_or("Command exited without a status")?;
    let stdout = String::from_utf8_lossy(&stdout_bytes);
    let stderr = String::from_utf8_lossy(&stderr_bytes);
    let mut text = String::new();
    text.push_str(stdout.trim_end());
    if !stderr.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str("[stderr]\n");
        text.push_str(stderr.trim_end());
    }
    if text.len() > OUTPUT_CAP {
        let mut end = OUTPUT_CAP;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n[output clipped]");
    }
    if status.success() {
        Ok(text)
    } else {
        let code = status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string());
        Err(format!("exit {code}: {text}"))
    }
}

/// Block the worker on the owner's Allow once / Deny decision for one
/// command. Cancellation wins; a dropped card counts as denial; an
/// unanswered card times out. One-shot: nothing is remembered.
pub fn await_approval(
    respond_rx: &std::sync::mpsc::Receiver<bool>,
    cancelled: &Arc<AtomicBool>,
    timeout: Duration,
) -> Result<bool, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err("Cancelled".into());
        }
        match respond_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(v) => return Ok(v),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if Instant::now() > deadline {
                    return Err("Command approval timed out".into());
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Ok(false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(cmd: &str, timeout: Duration) -> Result<String, String> {
        run_shell_command(
            std::path::Path::new("/tmp"),
            cmd,
            timeout,
            Arc::new(AtomicBool::new(false)),
        )
    }

    #[test]
    fn echo_roundtrip() {
        assert_eq!(
            run("printf 'hello agent'", Duration::from_secs(5)).unwrap(),
            "hello agent"
        );
    }

    #[test]
    fn failing_exit_is_err_with_output() {
        let error = run("echo oops >&2; exit 3", Duration::from_secs(5))
            .unwrap_err();
        assert!(error.starts_with("exit 3:"), "unexpected: {error}");
        assert!(error.contains("oops"));
    }

    #[test]
    fn timeout_kills() {
        let start = Instant::now();
        let error =
            run("sleep 30", Duration::from_millis(200)).unwrap_err();
        assert!(error.contains("timed out"), "unexpected: {error}");
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn output_is_capped_on_a_char_boundary() {
        let out = run("yes ABCDEFGH | head -c 200000", Duration::from_secs(10))
            .unwrap();
        assert!(out.len() < 66 * 1024);
        assert!(out.ends_with("[output clipped]"));
    }

    #[test]
    fn approval_allow_deny_disconnect() {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(true).unwrap();
        assert_eq!(
            await_approval(
                &rx,
                &Arc::new(AtomicBool::new(false)),
                Duration::from_secs(5)
            )
            .unwrap(),
            true
        );

        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(false).unwrap();
        assert_eq!(
            await_approval(
                &rx,
                &Arc::new(AtomicBool::new(false)),
                Duration::from_secs(5)
            )
            .unwrap(),
            false
        );

        // Dropped card = denial, not a hang.
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
        drop(tx);
        assert_eq!(
            await_approval(
                &rx,
                &Arc::new(AtomicBool::new(false)),
                Duration::from_secs(5)
            )
            .unwrap(),
            false
        );
    }

    #[test]
    fn approval_timeout_and_cancellation() {
        let (_tx, rx) = std::sync::mpsc::channel::<bool>();
        let start = Instant::now();
        let error = await_approval(
            &rx,
            &Arc::new(AtomicBool::new(false)),
            Duration::from_millis(300),
        )
        .unwrap_err();
        assert!(error.contains("timed out"), "unexpected: {error}");
        assert!(start.elapsed() < Duration::from_secs(5));

        let (_tx, rx) = std::sync::mpsc::channel::<bool>();
        assert_eq!(
            await_approval(
                &rx,
                &Arc::new(AtomicBool::new(true)),
                Duration::from_secs(5)
            )
            .unwrap_err(),
            "Cancelled"
        );
    }

    #[test]
    fn rejects_empty_and_oversized() {
        assert!(run("   ", Duration::from_secs(5)).is_err());
        assert!(run(&"x".repeat(5000), Duration::from_secs(5)).is_err());
    }
}
