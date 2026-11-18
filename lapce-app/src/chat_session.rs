//! Durable user-visible chat turns. File context is never persisted here.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
pub const MAX_TURNS: usize = 200;
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunState {
    Idle,
    Running,
    Cancelled,
    Error,
    Done,
    Interrupted,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedTurn {
    pub role: String,
    pub text: String,
    pub context: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub version: u32,
    pub id: String,
    pub turns: Vec<SavedTurn>,
    pub state: RunState,
}
impl Session {
    pub fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            version: 1,
            id: format!(
                "{nanos:032x}-{:08x}-{:08x}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ),
            turns: Vec::new(),
            state: RunState::Idle,
        }
    }
    pub fn recover(mut self) -> Self {
        // Each window resumes as a new session so two tabs cannot overwrite each other.
        self.id = Self::new().id;
        if self.state == RunState::Running {
            self.state = RunState::Interrupted;
        }
        self
    }
    fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.id.is_empty()
            || self.id.len() > 80
            || !self.id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
            || self.turns.len() > MAX_TURNS
            || self
                .turns
                .iter()
                .any(|t| !matches!(t.role.as_str(), "user" | "assistant"))
        {
            return Err("Unsupported or invalid chat session".into());
        }
        Ok(())
    }
}
pub fn workspace_directory(root: &Path, workspace: &str) -> PathBuf {
    root.join("chat-sessions")
        .join(format!("{:x}", Sha256::digest(workspace.as_bytes())))
}
fn private_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| e.to_string())?;
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err("Chat storage must be a real directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn save(directory: &Path, session: &Session) -> Result<(), String> {
    session.validate()?;
    let bytes = serde_json::to_vec(session).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Chat session is full. Start a new chat.".into());
    }
    private_directory(directory)?;
    let dest = directory.join(format!("{}.json", session.id));
    if let Ok(m) = fs::symlink_metadata(&dest) {
        if m.file_type().is_symlink() || !m.is_file() {
            return Err("Unsafe chat session path".into());
        }
    }
    let tmp = directory.join(format!(
        ".{}-{}.tmp",
        session.id,
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
/// Read-only native history browser. Never resumes a run or writes storage.
/// Enumerates at most 10,000 entries, examines the newest 100 JSON paths,
/// and returns at most 20 validated sessions without changing their ids/state.
pub fn recent(directory: &Path, limit: usize) -> Result<Vec<Session>, String> {
    let limit = limit.min(20);
    if limit == 0 || !directory.exists() { return Ok(Vec::new()); }
    let metadata = fs::symlink_metadata(directory).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Chat storage must be a real directory".into());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())?.take(10_000) {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) == Some("json")
            && entry.file_type().map_err(|e| e.to_string())?.is_file() {
            paths.push(path);
        }
    }
    if paths.is_empty() { return Ok(Vec::new()); }
    paths.sort();
    let mut sessions = Vec::new();
    for path in paths.into_iter().rev().take(100) {
        // O_NOFOLLOW closes the metadata/open symlink swap on Unix.
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let Ok(file) = options.open(&path) else { continue; };
        let Ok(metadata) = file.metadata() else { continue; };
        if !metadata.is_file() || metadata.len() > MAX_BYTES as u64 { continue; }
        let mut bytes = Vec::new();
        if file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes).is_err()
            || bytes.len() > MAX_BYTES { continue; }
        if let Ok(session) = serde_json::from_slice::<Session>(&bytes) {
            if session.validate().is_ok()
                && path.file_stem().and_then(|x| x.to_str()) == Some(session.id.as_str()) {
                sessions.push(session);
                if sessions.len() == limit { break; }
            }
        }
    }
    if sessions.is_empty() {
        Err("No readable chat session. Existing files were preserved.".into())
    } else { Ok(sessions) }
}
pub fn latest(directory: &Path) -> Result<Option<Session>, String> {
    if directory.exists() { private_directory(directory)?; }
    Ok(recent(directory, 1)?.into_iter().next().map(Session::recover))
}
/// Bounded plain text, not Markdown or an instruction to restore tool state.
pub fn preview(session: &Session) -> String {
    let mut lines = vec![format!("{:?} | {} saved turns. Preview only, not resumed.", session.state, session.turns.len())];
    if session.turns.len() > 6 { lines.push("Earlier turns omitted from preview.".into()); }
    for turn in session.turns.iter().skip(session.turns.len().saturating_sub(6)) {
        let mut text: String = turn.text.chars().take(400).collect();
        if turn.text.chars().count() > 400 { text.push_str("..."); }
        lines.push(format!("{}: {}", if turn.role == "user" { "You" } else { "Assistant" }, text));
    }
    lines.join("\n\n")
}
pub fn title(session: &Session) -> String {
    session.turns.iter().find(|t| t.role == "user")
        .map(|t| t.text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(80).collect())
        .filter(|s: &String| !s.is_empty()).unwrap_or_else(|| "Empty chat".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_history_is_bounded_and_does_not_recover_or_write() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..25 {
            let mut s = Session::new(); s.state = RunState::Running;
            s.turns.push(SavedTurn { role: "user".into(), text: format!("Question {i}"), context: String::new() });
            save(d.path(), &s).unwrap();
        }
        let before = fs::read_dir(d.path()).unwrap().count();
        let found = recent(d.path(), 1000).unwrap();
        assert_eq!(found.len(), 20); assert_eq!(found[0].state, RunState::Running);
        assert_eq!(found[0].id, recent(d.path(), 1).unwrap()[0].id);
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), before);
        assert!(recent(d.path(), 0).unwrap().is_empty());
    }
    #[test]
    fn native_preview_is_plain_bounded_unicode_and_inert() {
        let mut s = Session::new();
        for _ in 0..10 { s.turns.push(SavedTurn { role: "user".into(), text: "界".repeat(900), context: "not displayed".into() }); }
        let text = preview(&s);
        assert!(text.contains("Preview only, not resumed"));
        assert!(text.contains("Earlier turns omitted"));
        assert_eq!(text.matches("You:").count(), 6);
        assert!(text.chars().count() < 2600); assert!(!text.contains("not displayed"));
        assert_eq!(title(&s).chars().count(), 80);
    }
    #[test]
    fn native_history_skips_mismatched_ids_and_keeps_valid_older_rows() {
        let d = tempfile::tempdir().unwrap(); let s = Session::new(); save(d.path(), &s).unwrap();
        let mut other = s.clone(); other.id = "ffff".into();
        fs::write(d.path().join("ffffffff.json"), serde_json::to_vec(&other).unwrap()).unwrap();
        fs::write(d.path().join("fffffff.json"), "broken").unwrap();
        let found = recent(d.path(), 20).unwrap(); assert_eq!(found.len(), 1); assert_eq!(found[0].id, s.id);
    }
    #[test]
    fn private_round_trip_and_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Session::new();
        s.turns.push(SavedTurn {
            role: "user".into(),
            text: "hello\n世界".into(),
            context: "File: demo".into(),
        });
        s.state = RunState::Running;
        save(dir.path(), &s).unwrap();
        let r = latest(dir.path()).unwrap().unwrap();
        assert_eq!(r.state, RunState::Interrupted);
        assert_ne!(s.id, r.id);
        assert_eq!(r.turns[0].text, s.turns[0].text);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(dir.path().join(format!("{}.json", s.id)))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn workspace_isolation() {
        assert_ne!(
            workspace_directory(Path::new("/tmp"), "local:a"),
            workspace_directory(Path::new("/tmp"), "ssh:a")
        );
    }
    #[test]
    fn invalid_id_and_role_rejected() {
        let d = tempfile::tempdir().unwrap();
        let mut s = Session::new();
        s.id = "../escape".into();
        assert!(save(d.path(), &s).is_err());
        s.id = Session::new().id;
        s.turns.push(SavedTurn {
            role: "tool".into(),
            text: String::new(),
            context: String::new(),
        });
        assert!(save(d.path(), &s).is_err());
    }
    #[test]
    fn bounds_and_corrupt_files_preserved() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("bad.json"), "broken").unwrap();
        assert!(latest(d.path()).is_err());
        assert!(d.path().join("bad.json").exists());
        let mut s = Session::new();
        s.turns.push(SavedTurn {
            role: "user".into(),
            text: "x".repeat(MAX_BYTES),
            context: String::new(),
        });
        assert!(save(d.path(), &s).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_file_not_loaded_or_written() {
        use std::os::unix::fs::symlink;
        let d = tempfile::tempdir().unwrap();
        let s = Session::new();
        let target = d.path().join("outside");
        fs::write(&target, "safe").unwrap();
        symlink(&target, d.path().join(format!("{}.json", s.id))).unwrap();
        assert!(save(d.path(), &s).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "safe");
    }
}

/// A response belongs to exactly one run, and Stop invalidates it before killing IO.
#[derive(Clone, Default)]
pub struct RunGate(std::sync::Arc<AtomicU64>);
impl RunGate {
    pub fn begin(&self) -> u64 {
        self.0.fetch_add(1, Ordering::AcqRel) + 1
    }
    pub fn cancel(&self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
    pub fn accepts(&self, run: u64) -> bool {
        self.0.load(Ordering::Acquire) == run
    }
}
#[cfg(test)]
mod run_tests {
    use super::*;
    #[test]
    fn stale_updates_never_enter_new_run() {
        let gate = RunGate::default();
        let old = gate.begin();
        assert!(gate.accepts(old));
        gate.cancel();
        assert!(!gate.accepts(old));
        let new = gate.begin();
        assert!(!gate.accepts(old));
        assert!(gate.accepts(new));
    }
}
