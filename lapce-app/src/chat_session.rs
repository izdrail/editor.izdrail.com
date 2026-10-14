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
pub fn latest(directory: &Path) -> Result<Option<Session>, String> {
    if !directory.exists() {
        return Ok(None);
    }
    private_directory(directory)?;
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .take(10_000)
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) == Some("json")
            && entry.file_type().map_err(|e| e.to_string())?.is_file()
        {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        return Ok(None);
    }
    paths.sort();
    for path in paths.into_iter().rev().take(100) {
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if metadata.len() > MAX_BYTES as u64 || metadata.file_type().is_symlink() {
            continue;
        }
        let mut bytes = Vec::new();
        fs::File::open(&path)
            .map_err(|e| e.to_string())?
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            continue;
        }
        if let Ok(session) = serde_json::from_slice::<Session>(&bytes) {
            if session.validate().is_ok() {
                return Ok(Some(session.recover()));
            }
        }
    }
    Err("No readable chat session. Existing files were preserved.".into())
}
#[cfg(test)]
mod tests {
    use super::*;
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
