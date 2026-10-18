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
    /// Like `stage`, but applies only the owner-accepted hunks of each
    /// change. The digest still pins the full proposal; the masks are
    /// owner input from the review UI, never model input, and are
    /// validated against hunks recomputed from the pinned texts. A
    /// change with no accepted hunk is left untouched.
    pub fn stage_with_selection(
        &self,
        review: &Reviewed,
        current: &BTreeMap<String, BufferState>,
        masks: &[Vec<bool>],
    ) -> Result<Snapshot, String> {
        if self.digest()? != review.digest {
            return Err("Approval does not cover this proposal".into());
        }
        if masks.len() != self.changes.len() {
            return Err("Hunk selection does not match the proposal".into());
        }
        let mut before = BTreeMap::new();
        let mut applied = BTreeMap::new();
        for (change, mask) in self.changes.iter().zip(masks) {
            let hunks = hunks(&change.base.text, &change.proposed);
            if mask.len() != hunks.len() {
                return Err("Hunk selection does not match the proposal".into());
            }
            if mask.iter().all(|a| !a) {
                continue;
            }
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
                    text: merge_hunks(&change.base.text, &hunks, mask),
                },
            );
        }
        if before.is_empty() {
            return Err("No hunks accepted; nothing to apply".into());
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
/// One contiguous changed region between a base and a proposed
/// document, in whole lines with their newline terminators kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    /// 0-based line in the base where the region starts.
    pub base_start: usize,
    pub removed: Vec<String>,
    pub added: Vec<String>,
}
/// Split base/proposed into non-overlapping changed regions. Common
/// prefix/suffix trim, then a line LCS over the middle; middles too
/// large for the DP fall back to a single region (the behavior the
/// review view already shows). Deterministic, no allocation beyond
/// the region sizes.
pub fn hunks(base: &str, proposed: &str) -> Vec<Hunk> {
    let old: Vec<&str> = base.split_inclusive('\n').collect();
    let new: Vec<&str> = proposed.split_inclusive('\n').collect();
    if old == new {
        return Vec::new();
    }
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old_mid = &old[prefix..old.len() - suffix];
    let new_mid = &new[prefix..new.len() - suffix];
    const MAX_CELLS: usize = 1_000_000;
    if old_mid.is_empty() && new_mid.is_empty()
        || old_mid.len() * new_mid.len() > MAX_CELLS
    {
        return vec![Hunk {
            base_start: prefix,
            removed: old_mid.iter().map(|s| s.to_string()).collect(),
            added: new_mid.iter().map(|s| s.to_string()).collect(),
        }];
    }
    // lcs[i][j] = length of the LCS of old_mid[i..] and new_mid[j..].
    let (m, n) = (old_mid.len(), new_mid.len());
    let mut lcs = vec![vec![0u32; n + 1]; m + 1];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            lcs[i][j] = if old_mid[i] == new_mid[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    // Walk the LCS, emitting a region whenever a run of changes ends.
    let mut result = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    let mut current: Option<Hunk> = None;
    let mut flush = |current: &mut Option<Hunk>, result: &mut Vec<Hunk>| {
        if let Some(h) = current.take() {
            result.push(h);
        }
    };
    while i < m || j < n {
        let common = i < m && j < n && old_mid[i] == new_mid[j];
        if common {
            flush(&mut current, &mut result);
            i += 1;
            j += 1;
            continue;
        }
        let hunk = current.get_or_insert_with(|| Hunk {
            base_start: prefix + i,
            removed: Vec::new(),
            added: Vec::new(),
        });
        if i < m && (j == n || lcs[i + 1][j] >= lcs[i][j + 1]) {
            hunk.removed.push(old_mid[i].to_string());
            i += 1;
        } else {
            hunk.added.push(new_mid[j].to_string());
            j += 1;
        }
    }
    flush(&mut current, &mut result);
    result
}
/// Rebuild a document from its base lines and a per-hunk accept mask.
/// The selections must come from `hunks(base, proposed)` on the exact
/// base text; anything else is a caller bug and panics are avoided by
/// construction (indices only index into hunks, never the base).
pub fn merge_hunks(base: &str, hunks: &[Hunk], accepted: &[bool]) -> String {
    debug_assert_eq!(hunks.len(), accepted.len());
    let lines: Vec<&str> = base.split_inclusive('\n').collect();
    let mut out = String::with_capacity(base.len());
    let mut cursor = 0usize;
    for (hunk, accepted) in hunks.iter().zip(accepted.iter().copied()) {
        for line in &lines[cursor..hunk.base_start.min(lines.len())] {
            out.push_str(line);
        }
        cursor = hunk.base_start.min(lines.len());
        if accepted {
            for line in &hunk.added {
                out.push_str(line);
            }
        } else {
            for line in &hunk.removed {
                out.push_str(line);
            }
        }
        cursor += hunk.removed.len();
    }
    for line in &lines[cursor.min(lines.len())..] {
        out.push_str(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hunks_empty_base_is_one_insertion() {
        // New-file proposal: empty base diffs to a single all-added hunk.
        let hs = super::hunks("", "fn main() {}\nfn helper() {}\n");
        assert_eq!(hs.len(), 1);
        assert_eq!(hs[0].base_start, 0);
        assert!(hs[0].removed.is_empty());
        assert_eq!(hs[0].added, vec!["fn main() {}\n", "fn helper() {}\n"]);
        // Empty to empty is no change at all.
        assert!(super::hunks("", "").is_empty());
    }

    #[test]
    fn hunks_split_separate_regions() {
        let base = "a\nb\nc\nd\ne\nf\n";
        let proposed = "a\nB\nc\nd\nE\nf\n";
        let hs = hunks(base, proposed);
        assert_eq!(hs.len(), 2);
        assert_eq!(hs[0].base_start, 1);
        assert_eq!(hs[0].removed, vec!["b\n"]);
        assert_eq!(hs[0].added, vec!["B\n"]);
        assert_eq!(hs[1].base_start, 4);
        assert_eq!(hs[1].removed, vec!["e\n"]);
        assert_eq!(hs[1].added, vec!["E\n"]);
    }

    #[test]
    fn hunks_insertions_and_deletions() {
        let hs = hunks("a\nd\n", "a\nb\nc\nd\n");
        assert_eq!(hs.len(), 1);
        assert_eq!(hs[0].base_start, 1);
        assert!(hs[0].removed.is_empty());
        assert_eq!(hs[0].added, vec!["b\n", "c\n"]);
        let hs = hunks("a\nb\nc\nd\n", "a\nd\n");
        assert_eq!(hs.len(), 1);
        assert_eq!(hs[0].removed, vec!["b\n", "c\n"]);
        assert!(hs[0].added.is_empty());
        assert!(hunks("same\n", "same\n").is_empty());
    }

    #[test]
    fn merge_hunks_roundtrips_full_and_partial() {
        let base = "a\nb\nc\nd\ne\nf\n";
        let proposed = "a\nB\nc\nd\nE\nf\n";
        let hs = hunks(base, proposed);
        assert_eq!(merge_hunks(base, &hs, &[true, true]), proposed);
        assert_eq!(merge_hunks(base, &hs, &[false, false]), base);
        assert_eq!(merge_hunks(base, &hs, &[true, false]), "a\nB\nc\nd\ne\nf\n");
        assert_eq!(merge_hunks(base, &hs, &[false, true]), "a\nb\nc\nd\nE\nf\n");
    }

    #[test]
    fn merge_hunks_preserves_missing_final_newline() {
        let base = "one\ntwo";
        let proposed = "one\nTWO";
        let hs = hunks(base, proposed);
        assert_eq!(hs.len(), 1);
        assert_eq!(merge_hunks(base, &hs, &[true]), proposed);
        assert_eq!(merge_hunks(base, &hs, &[false]), base);
    }

    #[test]
    fn stage_with_selection_applies_only_accepted_hunks() {
        let mut current = BTreeMap::new();
        current.insert(
            "src/a.rs".into(),
            BufferState {
                revision: 3,
                text: "a\nb\nc\nd\ne\nf\n".into(),
            },
        );
        let proposal = Proposal {
            run_id: "run1".into(),
            changes: vec![Change {
                path: "src/a.rs".into(),
                base: current["src/a.rs"].clone(),
                proposed: "a\nB\nc\nd\nE\nf\n".into(),
            }],
        };
        let digest = proposal.digest().unwrap();
        let reviewed = proposal.reviewed(&digest).unwrap();
        // Accept the first hunk only.
        let snapshot = proposal
            .stage_with_selection(&reviewed, &current, &[vec![true, false]])
            .unwrap();
        assert_eq!(
            snapshot.applied["src/a.rs"].text,
            "a\nB\nc\nd\ne\nf\n"
        );
        assert_eq!(snapshot.applied["src/a.rs"].revision, 4);
        assert_eq!(
            snapshot.before["src/a.rs"].text,
            "a\nb\nc\nd\ne\nf\n"
        );
        // All accepted == the full stage result.
        let full = proposal
            .stage_with_selection(&reviewed, &current, &[vec![true, true]])
            .unwrap();
        assert_eq!(full.applied["src/a.rs"].text, "a\nB\nc\nd\nE\nf\n");
        // All rejected is an explicit no-op error, not a silent apply.
        assert!(proposal
            .stage_with_selection(&reviewed, &current, &[vec![false, false]])
            .unwrap_err()
            .contains("No hunks accepted"));
        // A wrong-shaped or stale selection never stages.
        assert!(proposal
            .stage_with_selection(&reviewed, &current, &[vec![true]])
            .unwrap_err()
            .contains("does not match"));
        assert!(proposal
            .stage_with_selection(&reviewed, &current, &[])
            .unwrap_err()
            .contains("does not match"));
        // A tampered proposal still fails the digest check first.
        let mut tampered = proposal.clone();
        tampered.changes[0].proposed = "a\nB\nc\nd\ne\nf\n".into();
        assert!(tampered
            .stage_with_selection(&reviewed, &current, &[vec![true, false]])
            .unwrap_err()
            .contains("Approval does not cover"));
    }

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
