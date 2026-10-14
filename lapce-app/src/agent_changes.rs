//! Revision-checked change proposals and owned-buffer checkpoints. No disk writes.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
};
const MAX_FILES: usize = 32;
const MAX_TEXT: usize = 256 * 1024;
const MAX_TOTAL: usize = 2 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BufferState {
    pub revision: u64,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub base: BufferState,
    pub proposed: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Proposal {
    pub run_id: String,
    pub changes: Vec<Change>,
}
#[derive(Clone, Debug)]
pub struct Reviewed {
    digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub before: BTreeMap<String, BufferState>,
    pub applied: BTreeMap<String, BufferState>,
}
fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path).components().all(|c| match c {
            Component::Normal(n) => n.to_str().is_some_and(|s| {
                let s = s.to_ascii_lowercase();
                !s.starts_with('.')
                    && !matches!(
                        s.as_str(),
                        "vendor" | "node_modules" | "credentials" | "secrets"
                    )
                    && !s.ends_with(".key")
                    && !s.ends_with(".pem")
            }),
            _ => false,
        })
}
impl Proposal {
    fn validate(&self) -> Result<(), String> {
        if self.run_id.is_empty()
            || self.run_id.len() > 128
            || self.changes.is_empty()
            || self.changes.len() > MAX_FILES
        {
            return Err("Invalid proposal size or run identity".into());
        }
        let mut seen = BTreeSet::new();
        let mut total = 0usize;
        for change in &self.changes {
            if !safe_path(&change.path) || !seen.insert(&change.path) {
                return Err("Unsafe or duplicate proposal path".into());
            }
            if change.base.text.len() > MAX_TEXT
                || change.proposed.len() > MAX_TEXT
                || change.base.text.contains('\0')
                || change.proposed.contains('\0')
            {
                return Err("Binary or oversized change".into());
            }
            total = total
                .checked_add(change.base.text.len() + change.proposed.len())
                .ok_or("Proposal size overflow")?;
            if total > MAX_TOTAL {
                return Err("Proposal too large".into());
            }
            if change.base.text == change.proposed {
                return Err("No-op change denied".into());
            }
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(self).map_err(|e| e.to_string())?)
        ))
    }
    /// Only the owner-review UI may call this. A model's approval text is not a review event.
    pub fn reviewed(&self, displayed_digest: &str) -> Result<Reviewed, String> {
        let digest = self.digest()?;
        if digest != displayed_digest {
            return Err("Displayed proposal changed; review again".into());
        }
        Ok(Reviewed { digest })
    }
    /// Returns a staged in-memory transaction only after every base is checked.
    /// The caller must hold document ownership and apply synchronously, one undo entry per document.
    pub fn stage(
        &self,
        review: &Reviewed,
        current: &BTreeMap<String, BufferState>,
    ) -> Result<Snapshot, String> {
        if self.digest()? != review.digest {
            return Err("Approval does not cover this proposal".into());
        }
        let mut before = BTreeMap::new();
        let mut applied = BTreeMap::new();
        for change in &self.changes {
            let state = current
                .get(&change.path)
                .ok_or("Open/validated document required; no implicit new file")?;
            if state != &change.base {
                return Err(format!(
                    "Document changed since review: {}",
                    change.path
                ));
            }
            before.insert(change.path.clone(), state.clone());
            applied.insert(
                change.path.clone(),
                BufferState {
                    revision: state
                        .revision
                        .checked_add(1)
                        .ok_or("Document revision overflow")?,
                    text: change.proposed.clone(),
                },
            );
        }
        Ok(Snapshot { before, applied })
    }
}
impl Snapshot {
    /// Restoring does not reset revisions, Git state or unrelated documents.
    /// Later user changes on any owned document cause a conflict before staging anything.
    pub fn restore(
        &self,
        current: &BTreeMap<String, BufferState>,
    ) -> Result<BTreeMap<String, BufferState>, String> {
        if self.before.len() != self.applied.len() || self.before.len() > MAX_FILES {
            return Err("Invalid checkpoint".into());
        }
        let mut restored = BTreeMap::new();
        for (path, applied) in &self.applied {
            if !safe_path(path) || current.get(path) != Some(applied) {
                return Err(format!("Later edit conflicts with checkpoint: {path}"));
            }
            let before = self
                .before
                .get(path)
                .ok_or("Invalid checkpoint ownership")?;
            if before.text.len() > MAX_TEXT || applied.text.len() > MAX_TEXT {
                return Err("Checkpoint too large".into());
            }
            restored.insert(
                path.clone(),
                BufferState {
                    revision: applied
                        .revision
                        .checked_add(1)
                        .ok_or("Document revision overflow")?,
                    text: before.text.clone(),
                },
            );
        }
        Ok(restored)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Proposal, BTreeMap<String, BufferState>) {
        let mut current = BTreeMap::new();
        current.insert(
            "app/User.php".into(),
            BufferState {
                revision: 2,
                text: "before".into(),
            },
        );
        current.insert(
            "unrelated.txt".into(),
            BufferState {
                revision: 1,
                text: "keep".into(),
            },
        );
        let p = Proposal {
            run_id: "run1".into(),
            changes: vec![Change {
                path: "app/User.php".into(),
                base: current["app/User.php"].clone(),
                proposed: "after 世界".into(),
            }],
        };
        (p, current)
    }
    #[test]
    fn reviewed_stage_and_restore_preserve_other_documents() {
        let (p, mut current) = fixture();
        let review = p.reviewed(&p.digest().unwrap()).unwrap();
        let s = p.stage(&review, &current).unwrap();
        current.extend(s.applied.clone());
        let restored = s.restore(&current).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored["app/User.php"].revision, 4);
        current.extend(restored);
        assert_eq!(current["unrelated.txt"].text, "keep");
    }
    #[test]
    fn stale_revision_or_text_and_changed_proposal_denied() {
        let (mut p, mut current) = fixture();
        let review = p.reviewed(&p.digest().unwrap()).unwrap();
        current.get_mut("app/User.php").unwrap().revision += 1;
        assert!(p.stage(&review, &current).is_err());
        let (_, current) = fixture();
        p.changes[0].proposed = "new plan".into();
        assert!(p.stage(&review, &current).is_err());
        assert!(p.reviewed("invented").is_err());
    }
    #[test]
    fn later_user_edit_blocks_restore() {
        let (p, mut current) = fixture();
        let s = p
            .stage(&p.reviewed(&p.digest().unwrap()).unwrap(), &current)
            .unwrap();
        current.extend(s.applied.clone());
        current.get_mut("app/User.php").unwrap().text = "user work".into();
        assert!(s.restore(&current).is_err());
        assert_eq!(current["app/User.php"].text, "user work");
    }
    #[test]
    fn multi_file_conflict_is_all_or_none() {
        let (mut p, mut current) = fixture();
        p.changes.push(Change {
            path: "unrelated.txt".into(),
            base: current["unrelated.txt"].clone(),
            proposed: "second".into(),
        });
        let r = p.reviewed(&p.digest().unwrap()).unwrap();
        current.get_mut("unrelated.txt").unwrap().text = "changed".into();
        assert!(p.stage(&r, &current).is_err());
        assert_eq!(current["app/User.php"].text, "before");
    }
    #[test]
    fn invalid_paths_duplicates_missing_and_bounds() {
        let (mut p, current) = fixture();
        p.changes[0].path = "../outside".into();
        assert!(p.digest().is_err());
        let (mut p, _) = fixture();
        p.changes.push(p.changes[0].clone());
        assert!(p.digest().is_err());
        let (mut p, _) = fixture();
        p.changes[0].proposed = "x".repeat(MAX_TEXT + 1);
        assert!(p.digest().is_err());
        let (p, _) = fixture();
        assert!(
            p.stage(&p.reviewed(&p.digest().unwrap()).unwrap(), &BTreeMap::new())
                .is_err()
        );
        assert_eq!(current["app/User.php"].text, "before");
    }
    #[test]
    fn checkpoint_serialization_keeps_owned_content() {
        let (p, current) = fixture();
        let s = p
            .stage(&p.reviewed(&p.digest().unwrap()).unwrap(), &current)
            .unwrap();
        let r: Snapshot =
            serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert_eq!(r.before, s.before);
        assert_eq!(r.applied, s.applied);
    }
}
