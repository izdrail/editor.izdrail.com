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

/// Current unix time in seconds (0 on clock failure), for stamps.
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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
    store_sessions(root, &sessions)
}

/// Remove one entry by session id; a missing id is a no-op (and
/// creates nothing on disk).
pub fn remove_session(root: &Path, session_id: &str) -> Result<(), String> {
    let mut sessions = load_sessions(root);
    let before = sessions.len();
    sessions.retain(|s| s.session_id != session_id);
    if sessions.len() == before {
        return Ok(());
    }
    store_sessions(root, &sessions)
}

fn store_sessions(root: &Path, sessions: &[StoredSession]) -> Result<(), String> {
    let dir = sessions_path(root);
    let dir = dir.parent().ok_or("bad sessions path")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let body = serde_json::to_vec_pretty(sessions).map_err(|e| e.to_string())?;
    let tmp = dir.join("sessions.json.tmp");
    std::fs::write(&tmp, body).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, sessions_path(root))
        .map_err(|e| format!("cannot store sessions: {e}"))
}

/// One stored session with the workspace root it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct RootedSession {
    pub root: PathBuf,
    pub session: StoredSession,
}

/// Merge sessions across workspace roots, newest first, capped at
/// MAX_SESSIONS. Duplicate roots (same path) are searched once; roots
/// that are missing or unreadable simply contribute nothing.
pub fn load_sessions_merged(roots: &[PathBuf]) -> Vec<RootedSession> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for root in roots {
        if !seen.insert(root.clone()) {
            continue;
        }
        for session in load_sessions(root) {
            out.push(RootedSession {
                root: root.clone(),
                session,
            });
        }
    }
    out.sort_by(|a, b| b.session.updated_unix.cmp(&a.session.updated_unix));
    out.truncate(MAX_SESSIONS);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_buckets_and_future_clamp() {
        let now = 10_000_000;
        assert_eq!(age_label(now, now), "0m");
        assert_eq!(age_label(now + 500, now), "0m");
        assert_eq!(age_label(now - 59 * 60, now), "59m");
        assert_eq!(age_label(now - 60 * 60, now), "1h");
        assert_eq!(age_label(now - 23 * 3600, now), "23h");
        assert_eq!(age_label(now - 24 * 3600, now), "1d");
        assert_eq!(age_label(now - 6 * 24 * 3600, now), "6d");
        assert_eq!(age_label(now - 7 * 24 * 3600, now), "1w");
        assert_eq!(age_label(now - 30 * 24 * 3600, now), "4w");
    }

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

    #[test]
    fn remove_deletes_and_roundtrips() {
        let r = root("remove");
        record_session(&r, entry("s-1", 100)).unwrap();
        record_session(&r, entry("s-2", 200)).unwrap();
        remove_session(&r, "s-1").unwrap();
        let sessions = load_sessions(&r);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "s-2");
        // Missing ids are a no-op and create nothing new.
        remove_session(&r, "nope").unwrap();
        assert_eq!(load_sessions(&r).len(), 1);
        // Removing the last entry leaves an empty, readable file.
        remove_session(&r, "s-2").unwrap();
        assert!(load_sessions(&r).is_empty());
    }

    #[test]
    fn merged_load_sorts_dedupes_and_skips_missing() {
        let a = root("merge-a");
        let b = root("merge-b");
        record_session(&a, entry("a-old", 100)).unwrap();
        record_session(&b, entry("b-new", 200)).unwrap();
        // Newest first across roots; a repeated root is searched once.
        let merged = load_sessions_merged(&[a.clone(), b.clone(), a.clone()]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].session.session_id, "b-new");
        assert_eq!(merged[1].session.session_id, "a-old");
        assert_eq!(merged[1].root, a.clone());
        // Missing roots contribute nothing.
        let merged = load_sessions_merged(&[b.join("nope"), a]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].session.session_id, "a-old");
    }
}

/// Short relative age for display: minutes under an hour, hours under a
/// day, days up to a week, then weeks. Future or equal stamps show "0m".
pub fn age_label(updated_unix: u64, now_unix: u64) -> String {
    let secs = now_unix.saturating_sub(updated_unix);
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h");
    }
    let days = hours / 24;
    if days < 7 {
        return format!("{days}d");
    }
    format!("{}w", days / 7)
}
