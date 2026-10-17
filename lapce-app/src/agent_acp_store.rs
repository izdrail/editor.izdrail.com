//! ACP session store: remembers agent session ids per workspace so a
//! later chat can resume them (agents advertising loadSession replay
//! the transcript themselves; we persist only the pointer, never the
//! conversation). Stored as `.lapce/acp/sessions.json`, capped at 64
//! entries, written via a temp file + rename so a crash cannot leave
//! half a file. Missing or malformed files read as empty.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Most sessions remembered per workspace.
pub const MAX_SESSIONS: usize = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredSession {
    pub session_id: String,
    /// The agent command that owns the session (resume with the same).
    pub agent: String,
    pub cwd: String,
    /// First user prompt line, trimmed; display only.
    pub title: String,
    pub updated_unix: u64,
}

pub fn sessions_path(root: &Path) -> PathBuf {
    root.join(".lapce").join("acp").join("sessions.json")
}

pub fn load_sessions(root: &Path) -> Vec<StoredSession> {
    let Ok(bytes) = std::fs::read(sessions_path(root)) else {
        return Vec::new();
    };
    let mut sessions: Vec<StoredSession> = serde_json::from_slice(&bytes).unwrap_or_default();
    sessions.sort_by(|a, b| b.updated_unix.cmp(&a.updated_unix));
    sessions.truncate(MAX_SESSIONS);
    sessions
}

/// Insert or refresh one entry (keyed by session id) and persist.
pub fn record_session(root: &Path, entry: StoredSession) -> Result<(), String> {
    let mut sessions = load_sessions(root);
    sessions.retain(|s| s.session_id != entry.session_id);
    sessions.insert(0, entry);
    sessions.truncate(MAX_SESSIONS);
    let dir = sessions_path(root);
    let dir = dir.parent().ok_or("bad sessions path")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let body = serde_json::to_vec_pretty(&sessions).map_err(|e| e.to_string())?;
    let tmp = dir.join("sessions.json.tmp");
    std::fs::write(&tmp, body).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, sessions_path(root))
        .map_err(|e| format!("cannot store sessions: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("acp-store-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entry(id: &str, stamp: u64) -> StoredSession {
        StoredSession {
            session_id: id.into(),
            agent: "opencode acp".into(),
            cwd: "/proj".into(),
            title: format!("fix thé röuter 🦀 {id}"),
            updated_unix: stamp,
        }
    }

    #[test]
    fn missing_and_malformed_read_as_empty() {
        let r = root("empty");
        assert!(load_sessions(&r).is_empty());
        std::fs::create_dir_all(r.join(".lapce").join("acp")).unwrap();
        std::fs::write(sessions_path(&r), "{ not json").unwrap();
        assert!(load_sessions(&r).is_empty());
    }

    #[test]
    fn record_upserts_sorts_and_caps() {
        let r = root("roundtrip");
        record_session(&r, entry("s-1", 100)).unwrap();
        record_session(&r, entry("s-2", 200)).unwrap();
        // Refreshing s-1 moves it to the top.
        record_session(&r, entry("s-1", 300)).unwrap();
        let sessions = load_sessions(&r);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].session_id, "s-1");
        assert_eq!(sessions[0].updated_unix, 300);
        assert_eq!(sessions[0].title, "fix thé röuter 🦀 s-1");
        for i in 0..(MAX_SESSIONS + 10) {
            record_session(&r, entry(&format!("bulk-{i}"), 1000 + i as u64)).unwrap();
        }
        let sessions = load_sessions(&r);
        assert_eq!(sessions.len(), MAX_SESSIONS);
        assert!(sessions.iter().all(|s| s.session_id != "s-2"));
    }
}
