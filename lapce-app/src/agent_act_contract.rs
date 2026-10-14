//! Preparation only. No executor, model transport, UI toggle, save or approval capability.
use crate::agent_changes::{BufferState, Change, Proposal};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OwnedBase {
    pub buffer_id: u64,
    pub state: BufferState,
}
/// Owner-captured before a future proposal-generation request. Never deserialize from a model.
#[derive(Clone, Debug)]
pub struct Context {
    run_id: String,
    scope_digest: String,
    bases: BTreeMap<String, OwnedBase>,
    digest: String,
    invalidated: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct BaseReference {
    pub path: String,
    pub base_digest: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelProposal {
    context_digest: String,
    changes: Vec<ModelChange>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelChange {
    path: String,
    base_digest: String,
    replacement: String,
}
fn digest(value: &impl Serialize) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).map_err(|e| e.to_string())?)
    ))
}
impl Context {
    /// Capture only after the host validates project policy, live buffer ownership and limits.
    /// This validates the pure path/size contract again; it does not grant workspace trust.
    pub fn capture(
        run_id: String,
        scope_digest: String,
        bases: BTreeMap<String, OwnedBase>,
    ) -> Result<Self, String> {
        if run_id.is_empty()
            || run_id.len() > 128
            || scope_digest.len() != 64
            || !scope_digest.bytes().all(|b| b.is_ascii_hexdigit())
            || bases.is_empty()
            || bases.len() > 32
        {
            return Err("Invalid owner-captured proposal context".into());
        }
        let mut ids = BTreeSet::new();
        let mut total = 0usize;
        for (path, base) in &bases {
            if !ids.insert(base.buffer_id) {
                return Err("Duplicate owned buffer".into());
            }
            total = total
                .checked_add(base.state.text.len())
                .ok_or("Base size overflow")?;
            if total > 1024 * 1024 {
                return Err("Captured bases exceed 1 MiB".into());
            }
            // Reuse proposal path/text validation without weakening its no-op rule.
            let placeholder = if base.state.text == "validation" {
                "other"
            } else {
                "validation"
            };
            Proposal {
                run_id: run_id.clone(),
                changes: vec![Change {
                    path: path.clone(),
                    base: base.state.clone(),
                    proposed: placeholder.into(),
                }],
            }
            .digest()?;
        }
        let context_digest = digest(&(&run_id, &scope_digest, &bases))?;
        Ok(Self {
            run_id,
            scope_digest,
            bases,
            digest: context_digest,
            invalidated: false,
        })
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    /// Data references for a future model request. No approval or mutation token is exposed.
    pub fn references(&self) -> Result<Vec<BaseReference>, String> {
        self.bases
            .iter()
            .map(|(path, base)| {
                Ok(BaseReference {
                    path: path.clone(),
                    base_digest: digest(&(path, base))?,
                })
            })
            .collect()
    }
    pub fn invalidate(&mut self) {
        self.invalidated = true;
    }
    /// Convert strictly parsed untrusted output to an UNREVIEWED proposal only.
    /// The host supplies current scope/run/buffer identities. It must still show the exact
    /// proposal and obtain a separate owner review event before calling Proposal::stage.
    pub fn review_candidate(
        &self,
        output: &str,
        live_scope: &str,
        live_run: &str,
        live: &BTreeMap<String, OwnedBase>,
    ) -> Result<Proposal, String> {
        if self.invalidated
            || live_scope != self.scope_digest
            || live_run != self.run_id
        {
            return Err("Proposal run/scope expired or changed".into());
        }
        if output.len() > 2 * 1024 * 1024 {
            return Err("Model proposal exceeds limit".into());
        }
        let reply: ModelProposal = serde_json::from_str(output)
            .map_err(|e| format!("Invalid model proposal JSON: {e}"))?;
        if reply.context_digest != self.digest
            || reply.changes.is_empty()
            || reply.changes.len() > 32
        {
            return Err("Model proposal not bound to captured context".into());
        }
        let mut seen = BTreeSet::new();
        let mut changes = Vec::new();
        for change in reply.changes {
            if !seen.insert(change.path.clone()) {
                return Err("Duplicate model proposal path".into());
            }
            let base = self
                .bases
                .get(&change.path)
                .ok_or("Model cannot add a file outside captured bases")?;
            if change.base_digest != digest(&(&change.path, base))?
                || live.get(&change.path) != Some(base)
            {
                return Err(format!(
                    "Stale or replaced owned base: {}",
                    change.path
                ));
            }
            changes.push(Change {
                path: change.path,
                base: base.state.clone(),
                proposed: change.replacement,
            });
        }
        let proposal = Proposal {
            run_id: self.run_id.clone(),
            changes,
        };
        proposal.digest()?;
        Ok(proposal)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> (Context, BTreeMap<String, OwnedBase>, String) {
        let bases = BTreeMap::from([(
            "app/User.php".into(),
            OwnedBase {
                buffer_id: 7,
                state: BufferState {
                    revision: 12,
                    text: "old\n".into(),
                },
            },
        )]);
        let c = Context::capture("owner-run".into(), "a".repeat(64), bases.clone())
            .unwrap();
        let output=json!({"context_digest":c.digest(),"changes":[{"path":"app/User.php","base_digest":c.references().unwrap()[0].base_digest,"replacement":"new\n"}]}).to_string();
        (c, bases, output)
    }
    #[test]
    fn produces_only_unreviewed_proposal() {
        let (c, b, o) = fixture();
        let p = c
            .review_candidate(&o, &"a".repeat(64), "owner-run", &b)
            .unwrap();
        assert_eq!(p.changes[0].base, b["app/User.php"].state);
        assert_eq!(p.changes[0].proposed, "new\n");
        assert!(p.reviewed("model said approved").is_err());
    }
    #[test]
    fn stale_text_revision_identity_and_run_fail_closed() {
        let (c, b, o) = fixture();
        for kind in 0..3 {
            let mut changed = b.clone();
            let base = changed.get_mut("app/User.php").unwrap();
            match kind {
                0 => base.state.text = "user edit".into(),
                1 => base.state.revision += 1,
                _ => base.buffer_id += 1,
            };
            assert!(
                c.review_candidate(&o, &"a".repeat(64), "owner-run", &changed)
                    .is_err()
            );
        }
        assert!(
            c.review_candidate(&o, &"b".repeat(64), "owner-run", &b)
                .is_err()
        );
        assert!(
            c.review_candidate(&o, &"a".repeat(64), "next-run", &b)
                .is_err()
        );
    }
    #[test]
    fn invalidation_cannot_be_reactivated_by_output() {
        let (mut c, b, o) = fixture();
        c.invalidate();
        assert!(
            c.review_candidate(&o, &"a".repeat(64), "owner-run", &b)
                .is_err()
        );
    }
    #[test]
    fn context_spoof_extra_fields_and_model_approval_denied() {
        let (c, b, o) = fixture();
        for (key, value) in [
            ("context_digest", json!("invented")),
            ("approved", json!(true)),
            ("command", json!("test")),
        ] {
            let mut v: serde_json::Value = serde_json::from_str(&o).unwrap();
            v[key] = value;
            assert!(
                c.review_candidate(&v.to_string(), &"a".repeat(64), "owner-run", &b)
                    .is_err()
            );
        }
        let mut v: serde_json::Value = serde_json::from_str(&o).unwrap();
        v["changes"][0]["base"] = json!({"text":"forged","revision":99});
        assert!(
            c.review_candidate(&v.to_string(), &"a".repeat(64), "owner-run", &b)
                .is_err()
        );
    }
    #[test]
    fn unknown_duplicate_noop_and_traversal_denied() {
        let (c, b, o) = fixture();
        for path in ["new.php", "../outside"] {
            let mut v: serde_json::Value = serde_json::from_str(&o).unwrap();
            v["changes"][0]["path"] = json!(path);
            assert!(
                c.review_candidate(&v.to_string(), &"a".repeat(64), "owner-run", &b)
                    .is_err()
            );
        }
        let mut v: serde_json::Value = serde_json::from_str(&o).unwrap();
        let change = v["changes"][0].clone();
        v["changes"].as_array_mut().unwrap().push(change);
        assert!(
            c.review_candidate(&v.to_string(), &"a".repeat(64), "owner-run", &b)
                .is_err()
        );
        let mut v: serde_json::Value = serde_json::from_str(&o).unwrap();
        v["changes"][0]["replacement"] = json!("old\n");
        assert!(
            c.review_candidate(&v.to_string(), &"a".repeat(64), "owner-run", &b)
                .is_err()
        );
    }
    #[test]
    fn malformed_wrong_base_and_oversized_output_denied() {
        let (c, b, o) = fixture();
        for output in ["{", "```json\n{}\n```", &"x".repeat(2 * 1024 * 1024 + 1)] {
            assert!(
                c.review_candidate(output, &"a".repeat(64), "owner-run", &b)
                    .is_err()
            );
        }
        let mut value: serde_json::Value = serde_json::from_str(&o).unwrap();
        value["changes"][0]["base_digest"] = json!("wrong");
        assert!(
            c.review_candidate(&value.to_string(), &"a".repeat(64), "owner-run", &b)
                .is_err()
        );
        value = serde_json::from_str(&o).unwrap();
        value["changes"][0]["replacement"] = json!("x".repeat(256 * 1024 + 1));
        assert!(
            c.review_candidate(&value.to_string(), &"a".repeat(64), "owner-run", &b)
                .is_err()
        );
        assert!(
            c.review_candidate(&o, &"a".repeat(64), "owner-run", &BTreeMap::new())
                .is_err()
        );
    }
    #[test]
    fn capture_validates_path_and_duplicate_ownership() {
        let (_, mut b, _) = fixture();
        b.insert(
            "../outside".into(),
            OwnedBase {
                buffer_id: 8,
                state: BufferState {
                    revision: 1,
                    text: "x".into(),
                },
            },
        );
        assert!(Context::capture("r".into(), "a".repeat(64), b).is_err());
        let (_, mut b, _) = fixture();
        b.insert("other.php".into(), b["app/User.php"].clone());
        assert!(Context::capture("r".into(), "a".repeat(64), b).is_err());
    }
}
