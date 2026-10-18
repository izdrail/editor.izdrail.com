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
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .spawn()
        .map_err(|e| format!("Could not start sh: {e}"))?;
    let deadline = Instant::now() + timeout;
    let out = loop {
        if cancelled.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Cancelled".into());
        }
        match child.try_wait() {
            Ok(Some(_)) => {
                break child
                    .wait_with_output()
                    .map_err(|e| format!("Could not collect output: {e}"))?;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "Command timed out after {}s",
                        timeout.as_secs().max(1)
                    ));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("Could not wait for command: {e}")),
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
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
    if out.status.success() {
        Ok(text)
    } else {
        let code = out
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string());
        Err(format!("exit {code}: {text}"))
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
    fn rejects_empty_and_oversized() {
        assert!(run("   ", Duration::from_secs(5)).is_err());
        assert!(run(&"x".repeat(5000), Duration::from_secs(5)).is_err());
    }
}
