//! Private bounded checkpoint journal, written before buffer edits. No auto recovery effects.
use crate::agent_changes::{BufferState, Snapshot};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::Path,
};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum State {
    Prepared,
    Applied,
    Restored,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub version: u32,
    pub proposal_digest: String,
    pub snapshot: Snapshot,
    pub state: State,
}
impl Checkpoint {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.proposal_digest.len() != 64
            || !self.proposal_digest.bytes().all(|c| c.is_ascii_hexdigit())
            || self.snapshot.before.is_empty()
            || self.snapshot.before.len() > 32
            || self.snapshot.before.len() != self.snapshot.applied.len()
        {
            return Err("Invalid checkpoint journal".into());
        }
        if self.snapshot.before.keys().ne(self.snapshot.applied.keys()) {
            return Err("Checkpoint ownership sets differ".into());
        }
        let p = crate::agent_changes::Proposal {
            run_id: "checkpoint-validation".into(),
            changes: self
                .snapshot
                .before
                .iter()
                .map(|(path, base)| crate::agent_changes::Change {
                    path: path.clone(),
                    base: base.clone(),
                    proposed: self
                        .snapshot
                        .applied
                        .get(path)
                        .map(|s| s.text.clone())
                        .unwrap_or_default(),
                })
                .collect(),
        };
        p.digest()?;
        Ok(())
    }
    /// Restart cannot compare old revision numbers. Require exact applied text on every
    /// owned buffer, then rebind revisions for the same explicit restore review.
    pub fn restore_staging(
        &self,
        current: &BTreeMap<String, BufferState>,
        recovered: bool,
    ) -> Result<BTreeMap<String, BufferState>, String> {
        self.validate()?;
        if self.state == State::Restored {
            return Err("Checkpoint already restored".into());
        }
        let mut snapshot = self.snapshot.clone();
        if recovered {
            for (path, applied) in &mut snapshot.applied {
                let now = current
                    .get(path)
                    .ok_or("Open all checkpoint files before restore review")?;
                if now.text != applied.text {
                    return Err(format!("Recovered checkpoint conflicts: {path}"));
                }
                *applied = now.clone();
            }
        }
        snapshot.restore(current)
    }
}
/// Bounded retained history. No automatic deletion of code-bearing recovery records.
pub fn capacity(directory: &Path) -> Result<(), String> {
    let entries = match fs::read_dir(directory) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
        Ok(entries) => entries,
    };
    if entries.take(33).count() >= 32 {
        return Err("Checkpoint history full (32 records). Archive/remove reviewed records before applying more edits".into());
    }
    Ok(())
}
pub fn latest(
    directory: &Path,
) -> Result<Option<(std::path::PathBuf, Checkpoint)>, String> {
    let entries = match fs::read_dir(directory) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
        Ok(entries) => entries,
    };
    let mut chosen = None;
    for (i, entry) in entries.enumerate() {
        if i >= 32 {
            return Err("Checkpoint history exceeds review limit".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            return Err(
                "Unexpected checkpoint record; preserved for manual review".into()
            );
        }
        let c = load(&path)?.ok_or("Checkpoint disappeared")?;
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .map_err(|e| e.to_string())?;
        if chosen.as_ref().is_none_or(|(time, _, _)| modified > *time) {
            chosen = Some((modified, path, c));
        }
    }
    Ok(chosen.map(|(_, p, c)| (p, c)))
}
#[cfg(target_os = "linux")]
fn directory(path: &Path, create: bool) -> Result<fs::File, String> {
    use rustix::fs::{Mode, OFlags, ResolveFlags, fchmod, openat2};
    let parent = path.parent().ok_or("Checkpoint directory missing")?;
    if create {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Pin the entire absolute parent path without following any substituted symlink.
    let absolute = if parent.is_absolute() {
        parent.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(parent)
    };
    let root = fs::File::open("/").map_err(|e| e.to_string())?;
    let file = fs::File::from(
        openat2(
            &root,
            absolute
                .strip_prefix("/")
                .map_err(|_| "Invalid checkpoint parent")?,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
        )
        .map_err(|e| format!("Secure checkpoint directory denied: {e}"))?,
    );
    use std::os::unix::fs::MetadataExt;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if meta.uid() != rustix::process::geteuid().as_raw() {
        return Err("Checkpoint directory not owned by current user".into());
    }
    fchmod(&file, Mode::from_raw_mode(0o700)).map_err(|e| e.to_string())?;
    Ok(file)
}
#[cfg(target_os = "linux")]
pub fn write(path: &Path, checkpoint: &Checkpoint) -> Result<(), String> {
    use rustix::fs::{
        AtFlags, FlockOperation, Mode, OFlags, ResolveFlags, flock, openat2,
        renameat, unlinkat,
    };
    use std::os::unix::fs::MetadataExt;
    checkpoint.validate()?;
    let bytes = serde_json::to_vec(checkpoint).map_err(|e| e.to_string())?;
    if bytes.len() > 5 * 1024 * 1024 {
        return Err("Checkpoint exceeds storage limit".into());
    }
    let dir = directory(path, true)?;
    flock(&dir, FlockOperation::NonBlockingLockExclusive)
        .map_err(|_| "Checkpoint storage busy".to_string())?;
    let name = path.file_name().ok_or("Invalid checkpoint name")?;
    let flags =
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV;
    match openat2(
        &dir,
        name,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
        flags,
    ) {
        Ok(fd) => {
            let m = fs::File::from(fd).metadata().map_err(|e| e.to_string())?;
            if !m.is_file()
                || m.nlink() != 1
                || m.uid() != rustix::process::geteuid().as_raw()
            {
                return Err("Unsafe checkpoint destination".into());
            }
        }
        Err(rustix::io::Errno::NOENT) => {
            capacity(path.parent().ok_or("Checkpoint parent missing")?)?;
        }
        Err(e) => return Err(format!("Unsafe checkpoint path: {e}")),
    }
    let temp = format!(
        "pending-{}-{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let mut file = fs::File::from(
        openat2(
            &dir,
            &temp,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
            flags,
        )
        .map_err(|e| e.to_string())?,
    );
    let result = (|| {
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        renameat(&dir, &temp, &dir, name).map_err(|e| e.to_string())?;
        dir.sync_all().map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = unlinkat(&dir, &temp, AtFlags::empty());
    }
    result
}
#[cfg(not(target_os = "linux"))]
pub fn write(_: &Path, _: &Checkpoint) -> Result<(), String> {
    Err("Secure checkpoints are Linux-only".into())
}
#[cfg(target_os = "linux")]
pub fn load(path: &Path) -> Result<Option<Checkpoint>, String> {
    use rustix::fs::{Mode, OFlags, ResolveFlags, openat2};
    use std::os::unix::fs::MetadataExt;
    let dir = directory(path, false)?;
    let fd = match openat2(
        &dir,
        path.file_name().ok_or("Invalid checkpoint name")?,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
    ) {
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(e) => return Err(format!("Secure checkpoint read denied: {e}")),
        Ok(fd) => fd,
    };
    let file = fs::File::from(fd);
    let m = file.metadata().map_err(|e| e.to_string())?;
    if !m.is_file()
        || m.nlink() != 1
        || m.len() > 5 * 1024 * 1024
        || m.uid() != rustix::process::geteuid().as_raw()
        || m.mode() & 0o077 != 0
    {
        return Err("Unsafe, non-private or oversized checkpoint".into());
    }
    let mut bytes = Vec::new();
    file.take(5 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 5 * 1024 * 1024 {
        return Err("Oversized checkpoint".into());
    }
    let c: Checkpoint = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Checkpoint unreadable; preserved: {e}"))?;
    c.validate()?;
    Ok(Some(c))
}
#[cfg(not(target_os = "linux"))]
pub fn load(_: &Path) -> Result<Option<Checkpoint>, String> {
    Err("Secure checkpoints are Linux-only".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Checkpoint {
        let base = BufferState {
            revision: 1,
            text: "old".into(),
        };
        let applied = BufferState {
            revision: 2,
            text: "new".into(),
        };
        Checkpoint {
            version: 1,
            proposal_digest: "a".repeat(64),
            snapshot: Snapshot {
                before: BTreeMap::from([("file.php".into(), base)]),
                applied: BTreeMap::from([("file.php".into(), applied)]),
            },
            state: State::Applied,
        }
    }
    #[test]
    fn retention_and_corrupt_history_fail_closed() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.json");
        write(&p, &fixture()).unwrap();
        assert!(latest(d.path()).unwrap().is_some());
        fs::write(d.path().join("b.json"), "broken").unwrap();
        assert!(latest(d.path()).is_err());
        for n in 0..30 {
            fs::write(d.path().join(format!("{n}.json")), "broken").unwrap();
        }
        assert!(capacity(d.path()).is_err());
    }
    #[test]
    fn mismatched_ownership_denied() {
        let mut c = fixture();
        let state = c.snapshot.applied.remove("file.php").unwrap();
        c.snapshot.applied.insert("other.php".into(), state);
        assert!(c.validate().is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn private_modes_and_parent_symlinks_and_hardlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let d = tempfile::tempdir().unwrap();
        let real = d.path().join("real");
        let p = real.join("c.json");
        write(&p, &fixture()).unwrap();
        assert_eq!(fs::metadata(&p).unwrap().permissions().mode() & 0o077, 0);
        let alias = d.path().join("alias");
        symlink(&real, &alias).unwrap();
        assert!(load(&alias.join("c.json")).is_err());
        assert!(write(&alias.join("other.json"), &fixture()).is_err());
        fs::hard_link(&p, real.join("second.json")).unwrap();
        assert!(load(&p).is_err());
        assert!(write(&p, &fixture()).is_err());
    }
    #[test]
    fn private_roundtrip_and_restart_review() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("checkpoint.json");
        let c = fixture();
        write(&p, &c).unwrap();
        let c = load(&p).unwrap().unwrap();
        let now = BTreeMap::from([(
            "file.php".into(),
            BufferState {
                revision: 40,
                text: "new".into(),
            },
        )]);
        assert!(c.restore_staging(&now, false).is_err());
        let restored = c.restore_staging(&now, true).unwrap();
        assert_eq!(restored["file.php"].text, "old");
        assert_eq!(restored["file.php"].revision, 41);
    }
    #[test]
    fn conflict_corruption_and_restored_fail_closed() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("c.json");
        fs::write(&p, "broken").unwrap();
        assert!(load(&p).is_err());
        assert!(p.exists());
        let mut c = fixture();
        let now = BTreeMap::from([(
            "file.php".into(),
            BufferState {
                revision: 2,
                text: "user work".into(),
            },
        )]);
        assert!(c.restore_staging(&now, true).is_err());
        c.state = State::Restored;
        assert!(c.restore_staging(&c.snapshot.applied, true).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_destination_denied() {
        let d = tempfile::tempdir().unwrap();
        let outside = d.path().join("outside");
        fs::write(&outside, "keep").unwrap();
        let p = d.path().join("c.json");
        std::os::unix::fs::symlink(&outside, &p).unwrap();
        assert!(write(&p, &fixture()).is_err());
        assert_eq!(fs::read_to_string(outside).unwrap(), "keep");
    }
}
