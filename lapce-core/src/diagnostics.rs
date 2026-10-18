//! Opt-in diagnostics: debug mode switch, crash reports and log pruning.
//!
//! Nothing here does anything unless debug mode is enabled with `--debug`
//! or `LAPCE_DEBUG=1`, so default behaviour is unchanged.

use std::{
    io::Write,
    panic::PanicHookInfo,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::directory::Directory;

/// Environment variable that turns debug mode on (`1`, `true`, `yes`, `on`).
pub const DEBUG_ENV: &str = "LAPCE_DEBUG";
/// File name of the crash report log inside the logs directory.
pub const CRASH_LOG_NAME: &str = "crash.log";
/// The crash log is rotated to `crash.log.1` once it grows past this size.
pub const MAX_CRASH_LOG_BYTES: u64 = 2 * 1024 * 1024;
/// Total size cap for rotated `*.log` files in the logs directory.
pub const MAX_LOGS_TOTAL_BYTES: u64 = 200 * 1024 * 1024;

static DEBUG: AtomicBool = AtomicBool::new(false);

fn env_enabled() -> bool {
    std::env::var(DEBUG_ENV)
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// Turn debug mode on for this process. Also exports `LAPCE_DEBUG=1` so
/// child processes (the re-spawned editor, remote proxies) inherit it.
pub fn enable_debug() {
    DEBUG.store(true, Ordering::Relaxed);
    // Called early on startup, before other threads are spawned.
    unsafe { std::env::set_var(DEBUG_ENV, "1") };
}

/// Whether debug mode is on (`--debug` given or `LAPCE_DEBUG` set).
pub fn is_debug() -> bool {
    DEBUG.load(Ordering::Relaxed) || env_enabled()
}

/// Path of the crash log, if the logs directory is available.
pub fn crash_log_path() -> Option<PathBuf> {
    Directory::logs_directory().map(|d| d.join(CRASH_LOG_NAME))
}

fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (Howard Hinnant), UTC.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Build the text of one crash report.
pub fn format_crash_report(
    process: &str,
    thread: &str,
    message: &str,
    location: Option<&str>,
    backtrace: &str,
) -> String {
    format!(
        "==== crash report {} ====\nversion: {} ({})\nprocess: {process} (pid {})\nos: {} {}\nthread: {thread}\nmessage: {message}\nlocation: {}\nbacktrace:\n{backtrace}\n\n",
        timestamp(),
        crate::meta::VERSION,
        crate::meta::RELEASE.as_ref(),
        std::process::id(),
        std::env::consts::OS,
        std::env::consts::ARCH,
        location.unwrap_or("<unknown>"),
    )
}

/// Append a report to `path`, rotating to `<path>.1` when over the size cap.
pub fn append_crash_report(path: &Path, report: &str) -> std::io::Result<()> {
    if std::fs::metadata(path)
        .map(|m| m.len() >= MAX_CRASH_LOG_BYTES)
        .unwrap_or(false)
    {
        let mut rotated = path.as_os_str().to_owned();
        rotated.push(".1");
        let _ = std::fs::rename(path, rotated);
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(report.as_bytes())?;
    file.flush()
}

fn panic_text(info: &PanicHookInfo<'_>) -> String {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Write a crash report for `info` to `path`. The backtrace is always
/// captured, regardless of `RUST_BACKTRACE`.
pub fn record_panic_to(path: &Path, process: &str, info: &PanicHookInfo<'_>) {
    let thread = std::thread::current();
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
    let backtrace = std::backtrace::Backtrace::force_capture().to_string();
    let report = format_crash_report(
        process,
        thread.name().unwrap_or("<unnamed>"),
        &panic_text(info),
        location.as_deref(),
        &backtrace,
    );
    if let Err(err) = append_crash_report(path, &report) {
        eprintln!("failed to write crash report to {}: {err}", path.display());
    }
}

/// Record a panic to the default crash log. Does nothing outside debug mode.
pub fn record_panic(process: &str, info: &PanicHookInfo<'_>) {
    if !is_debug() {
        return;
    }
    if let Some(path) = crash_log_path() {
        record_panic_to(&path, process, info);
    }
}

/// Install a panic hook that writes crash reports to `path` and then runs
/// the previously installed hook.
pub fn install_panic_hook_at(path: PathBuf, process: &'static str) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        record_panic_to(&path, process, info);
        previous(info);
    }));
}

/// Install the crash-report panic hook on the default crash log. Does
/// nothing outside debug mode.
pub fn install_panic_hook(process: &'static str) {
    if !is_debug() {
        return;
    }
    if let Some(path) = crash_log_path() {
        install_panic_hook_at(path, process);
    }
}

/// Delete the oldest `*.log*` files in `dir` until the total size is at most
/// `max_total` bytes. Returns the number of files removed.
pub fn prune_logs(dir: &Path, max_total: u64) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".log"))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            meta.is_file().then(|| {
                (meta.modified().unwrap_or(UNIX_EPOCH), meta.len(), e.path())
            })
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort_by_key(|f| f.0);
    let mut removed = 0;
    // Never delete the newest file, even if it alone is over the cap.
    let keep = files.len().saturating_sub(1);
    for (_, len, path) in files.into_iter().take(keep) {
        if total <= max_total {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
            removed += 1;
        }
    }
    removed
}

/// Last `max_lines` lines of the crash log, capped at 8 KiB. None when
/// there is no crash log or it is empty.
pub fn crash_log_tail(max_lines: usize) -> Option<String> {
    let path = crash_log_path()?;
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() == 0 {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let max_lines = max_lines.min(40);
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    let mut tail: String = lines[start..].join("\n");
    if tail.len() > 8192 {
        // Keep the end; the cut point is at a char boundary because we
        // walk forward from the byte estimate.
        let mut keep_from = tail.len() - 8192;
        while !tail.is_char_boundary(keep_from) {
            keep_from += 1;
        }
        tail = tail[keep_from..].to_string();
    }
    if tail.trim().is_empty() { None } else { Some(tail) }
}

/// The copyable diagnostics report shown in the About popup. Best-effort
/// and bounded: every line is short and the crash tail is capped.
pub fn diagnostics_report(
    version: &str,
    ai_autocomplete: (&str, &str, &str),
    ai_chat: (&str, &str, &str),
    crash_tail: Option<String>,
) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(1024);
    let path = |p: Option<PathBuf>| {
        p.map(|p| p.display().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    };
    let _ = writeln!(s, "Lapce diagnostics");
    let _ = writeln!(s, "version: {version}");
    let _ = writeln!(
        s,
        "os: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let _ = writeln!(
        s,
        "debug mode: {} (enable with --debug or {}=1)",
        if is_debug() { "on" } else { "off" },
        DEBUG_ENV
    );
    let _ = writeln!(s, "config dir: {}", path(Directory::config_directory()));
    let _ = writeln!(s, "logs dir: {}", path(Directory::logs_directory()));
    let _ = writeln!(s, "crash log: {}", path(crash_log_path()));
    let _ = writeln!(
        s,
        "proxy: connected (this window is rendered over the proxy channel)"
    );
    let (b, e, m) = ai_autocomplete;
    let _ = writeln!(s, "ai autocomplete: backend={b} endpoint={e} model={m}");
    let (b, e, m) = ai_chat;
    let _ = writeln!(s, "ai chat: backend={b} endpoint={e} model={m}");
    match crash_tail {
        Some(t) if !t.trim().is_empty() => {
            let _ = writeln!(s, "recent crash reports:");
            let _ = writeln!(s, "{t}");
        }
        _ => {
            let _ = writeln!(s, "recent crash reports: none");
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_contains_fields() {
        let r = format_crash_report("lapce", "main", "boom", Some("a.rs:1:2"), "bt");
        assert!(r.contains("message: boom"));
        assert!(r.contains("thread: main"));
        assert!(r.contains("location: a.rs:1:2"));
        assert!(r.contains("backtrace:\nbt"));
    }

    #[test]
    fn diagnostics_report_has_all_sections() {
        let r = diagnostics_report(
            "1.0.0+test.abc1234",
            ("ollama", "https://ai.izdrail.com", "qwen"),
            ("local", "", "qwen-chat-1.5b"),
            None,
        );
        assert!(r.contains("version: 1.0.0+test.abc1234"));
        assert!(r.contains("os: "));
        assert!(r.contains("debug mode:"));
        assert!(r.contains("config dir:"));
        assert!(r.contains("logs dir:"));
        assert!(r.contains("crash log:"));
        assert!(r.contains("proxy: connected"));
        assert!(r.contains(
            "ai autocomplete: backend=ollama endpoint=https://ai.izdrail.com model=qwen"
        ));
        assert!(r.contains("recent crash reports: none"));
        assert!(r.len() < 2048, "report without crash tail stays small");
        let with_crash = diagnostics_report(
            "v",
            ("b", "e", "m"),
            ("b", "e", "m"),
            Some("crash 1\ncrash 2".to_string()),
        );
        assert!(with_crash.contains("recent crash reports:\ncrash 1\ncrash 2"));
    }

    #[test]
    fn timestamp_shape() {
        let t = timestamp();
        assert_eq!(t.len(), 20);
        assert!(t.ends_with('Z') && t.as_bytes()[10] == b'T');
    }

    #[test]
    fn crash_log_appends_and_rotates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CRASH_LOG_NAME);
        append_crash_report(&path, "one\n").unwrap();
        append_crash_report(&path, "two\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\ntwo\n");
        let big = "x".repeat(MAX_CRASH_LOG_BYTES as usize);
        std::fs::write(&path, big).unwrap();
        append_crash_report(&path, "fresh\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "fresh\n");
        assert!(dir.path().join("crash.log.1").exists());
    }

    #[test]
    fn prune_removes_oldest_but_keeps_newest() {
        let dir = tempfile::tempdir().unwrap();
        for (i, name) in ["a.log", "b.log", "c.log"].iter().enumerate() {
            let p = dir.path().join(name);
            std::fs::write(&p, vec![b'x'; 100]).unwrap();
            let t = UNIX_EPOCH + std::time::Duration::from_secs(1000 + i as u64);
            std::fs::File::options()
                .write(true)
                .open(&p)
                .unwrap()
                .set_modified(t)
                .unwrap();
        }
        std::fs::write(dir.path().join("notes.txt"), "keep").unwrap();
        assert_eq!(prune_logs(dir.path(), 150), 2);
        assert!(!dir.path().join("a.log").exists());
        assert!(!dir.path().join("b.log").exists());
        assert!(dir.path().join("c.log").exists());
        assert!(dir.path().join("notes.txt").exists());
        assert_eq!(prune_logs(dir.path(), 0), 0);
    }
}
