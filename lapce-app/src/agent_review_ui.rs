//! Owner-only diff review and buffer application. Never parses a chat reply as approval.
use crate::{
    agent_changes::{BufferState, Change, Proposal},
    agent_checkpoint::{self, Checkpoint, State},
    doc::Doc,
    window_tab::WindowTabData,
};
use floem::{
    View,
    reactive::{
        RwSignal, Scope, SignalGet, SignalUpdate, SignalWith, create_effect,
        create_rw_signal,
    },
    style::CursorStyle,
    views::{Decorators, container, dyn_stack, h_stack, label, scroll, v_stack},
};
use lapce_core::{
    buffer::rope_text::RopeText, editor::EditType, selection::Selection,
};
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf, rc::Rc};
#[derive(Clone)]
pub struct ReviewData {
    pub proposal: RwSignal<Option<Proposal>>,
    pub checkpoint: RwSignal<Option<Checkpoint>>,
    pub recovered: RwSignal<bool>,
    pub error: RwSignal<String>,
    pub notice: RwSignal<String>,
    directory: Option<PathBuf>,
    path: RwSignal<Option<PathBuf>>,
    owners: RwSignal<BTreeMap<String, u64>>,
    checkpoint_owners: RwSignal<BTreeMap<String, u64>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Draft {
    changes: Vec<DraftChange>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DraftChange {
    path: String,
    replacement: String,
}
impl ReviewData {
    pub fn new(cx: Scope, path: Option<PathBuf>) -> Self {
        let loaded = path
            .as_ref()
            .map(|p| agent_checkpoint::latest(p))
            .transpose();
        let (checkpoint, journal, error) = match loaded {
            Ok(Some(Some((p, c)))) => (Some(c), Some(p), String::new()),
            Ok(_) => (None, None, String::new()),
            Err(e) => (None, None, e),
        };
        let recovered = checkpoint.is_some();
        Self {
            proposal: cx.create_rw_signal(None),
            checkpoint: cx.create_rw_signal(checkpoint),
            recovered: cx.create_rw_signal(recovered),
            error: cx.create_rw_signal(error),
            notice: cx.create_rw_signal(String::new()),
            directory: path,
            path: cx.create_rw_signal(journal),
            owners: cx.create_rw_signal(BTreeMap::new()),
            checkpoint_owners: cx.create_rw_signal(BTreeMap::new()),
        }
    }
    fn root(tab: &WindowTabData) -> Result<PathBuf, String> {
        if !tab.workspace.kind.is_local() {
            return Err("Reviewed edits require a local project".into());
        }
        tab.workspace
            .path
            .as_ref()
            .ok_or("Open a local project")?
            .canonicalize()
            .map_err(|e| e.to_string())
    }
    fn docs(
        tab: &WindowTabData,
        paths: impl Iterator<Item = String>,
    ) -> Result<BTreeMap<String, Rc<Doc>>, String> {
        if tab.common.config.get_untracked().editor.autosave_interval > 0 {
            return Err("Disable editor autosave before preparing or restoring reviewed edits; this milestone never saves source files".into());
        }
        let root = Self::root(tab)?;
        #[cfg(not(target_os = "linux"))]
        return Err("Secure reviewed-edit validation is Linux-only".into());
        #[cfg(target_os = "linux")]
        let reader = crate::agent_read::ReadRoot::new(&root, true)?;
        let all = tab.main_split.docs.get_untracked();
        let mut result = BTreeMap::new();
        let mut identities = std::collections::BTreeSet::new();
        for relative in paths {
            #[cfg(target_os = "linux")]
            reader.read(std::path::Path::new(&relative))?;
            let path = root.join(&relative);
            let doc = all
                .get(&path)
                .cloned()
                .or_else(|| {
                    all.iter().find_map(|(p, d)| {
                        (p.canonicalize().ok().as_ref() == Some(&path))
                            .then(|| d.clone())
                    })
                })
                .ok_or_else(|| format!("Open file before review: {relative}"))?;
            if !doc.loaded.get_untracked()
                || doc.content.with_untracked(|c| c.read_only())
            {
                return Err(format!("Document not writable/loaded: {relative}"));
            }
            if !identities.insert(doc.buffer_id.0) {
                return Err("Duplicate owned buffer denied".into());
            }
            result.insert(relative, doc);
        }
        Ok(result)
    }
    fn states(docs: &BTreeMap<String, Rc<Doc>>) -> BTreeMap<String, BufferState> {
        docs.iter()
            .map(|(p, d)| {
                (
                    p.clone(),
                    d.buffer.with_untracked(|b| BufferState {
                        revision: b.rev(),
                        text: b.to_string(),
                    }),
                )
            })
            .collect()
    }
    pub fn prepare(&self, tab: &WindowTabData, json: &str) -> Result<(), String> {
        if tab.chat.busy.get_untracked() {
            return Err("Stop the current run before reviewing edits".into());
        }
        let changes = parse_draft(json)?;
        let docs = Self::docs(tab, changes.iter().map(|c| c.path.clone()))?;
        let current = Self::states(&docs);
        if docs.iter().any(|(p, d)| {
            d.buffer.with_untracked(|b| {
                b.line_ending()
                    .normalize(&lapce_xi_rope::Rope::from(current[p].text.as_str()))
                    .to_string()
            }) != current[p].text
        }) {
            return Err("Normalize mixed line endings manually before preparing review; checkpoint restore must preserve the exact base".into());
        }
        let proposal = Proposal {
            run_id: tab.chat.session.get_untracked().id,
            changes: changes
                .into_iter()
                .map(|c| Change {
                    base: current[&c.path].clone(),
                    path: c.path.clone(),
                    proposed: docs[&c.path].buffer.with_untracked(|b| {
                        b.line_ending()
                            .normalize(&lapce_xi_rope::Rope::from(
                                c.replacement.as_str(),
                            ))
                            .to_string()
                    }),
                })
                .collect(),
        };
        proposal.digest()?;
        self.owners.set(
            docs.iter()
                .map(|(p, d)| (p.clone(), d.buffer_id.0))
                .collect(),
        );
        self.proposal.set(Some(proposal));
        self.error.set(String::new());
        self.notice.set(
            "Review every before/after file below. Nothing changed yet.".into(),
        );
        Ok(())
    }
    fn journal_path(&self) -> Result<PathBuf, String> {
        self.path
            .get_untracked()
            .ok_or_else(|| "Checkpoint storage unavailable".to_string())
    }
    pub fn apply(
        &self,
        tab: &WindowTabData,
        displayed: &Proposal,
    ) -> Result<(), String> {
        if tab.chat.busy.get_untracked() {
            return Err("Stop current run first".into());
        }
        let current = self
            .proposal
            .get_untracked()
            .ok_or("Proposal no longer pending")?;
        let digest = displayed.digest()?;
        if current.digest()? != digest {
            return Err("Proposal changed; review again".into());
        }
        let docs = Self::docs(tab, current.changes.iter().map(|c| c.path.clone()))?;
        {
            let owners = self.owners.get_untracked();
            if docs
                .iter()
                .any(|(p, d)| owners.get(p) != Some(&d.buffer_id.0))
            {
                return Err(
                    "Owned document was closed/reopened; prepare review again"
                        .into(),
                );
            }
        }
        let states = Self::states(&docs);
        let reviewed = current.reviewed(&digest)?;
        let mut snapshot = current.stage(&reviewed, &states)?;
        let staged = staged_buffers(&docs, &snapshot.applied)?;
        snapshot.applied = staged
            .iter()
            .map(|(p, (buffer, _))| {
                (
                    p.clone(),
                    BufferState {
                        revision: buffer.rev(),
                        text: buffer.to_string(),
                    },
                )
            })
            .collect();
        let directory = self
            .directory
            .as_ref()
            .ok_or("Checkpoint storage unavailable; cannot apply")?;
        // Retain earlier checkpoints rather than silently overwriting another window's recovery.
        agent_checkpoint::capacity(directory)?;
        agent_checkpoint::latest(directory)?; // Corrupt prior recovery records block new changes.
        let path = directory.join(format!("{digest}.json"));
        if path.exists() {
            return Err("Checkpoint already exists; review a new proposal".into());
        }

        let mut checkpoint = Checkpoint {
            version: 1,
            proposal_digest: digest,
            snapshot,
            state: State::Prepared,
        };
        agent_checkpoint::write(&path, &checkpoint)?;
        self.path.set(Some(path.clone()));
        self.checkpoint.set(Some(checkpoint.clone()));
        self.recovered.set(false);
        self.checkpoint_owners.set(self.owners.get_untracked());
        // One UI callback owns all loaded documents. No await, save or file create occurs.
        commit_buffers(&docs, staged);
        checkpoint.snapshot.applied = Self::states(&docs);
        checkpoint.state = State::Applied;
        self.checkpoint.set(Some(checkpoint.clone()));
        self.recovered.set(false);
        self.proposal.set(None);
        if let Err(e) = agent_checkpoint::write(&path, &checkpoint) {
            self.notice.set("Buffers changed but applied journal update failed. Keep window open; checkpoint is retained for review.".into());
            return Err(e);
        }
        self.notice.set("Applied to open buffers only, not saved. One undo per document; Restore checkpoint reverts all owned buffers if unchanged.".into());
        Ok(())
    }
    pub fn restore(
        &self,
        tab: &WindowTabData,
        displayed: &Checkpoint,
    ) -> Result<(), String> {
        if tab.chat.busy.get_untracked() {
            return Err("Stop current run first".into());
        }
        let checkpoint = self.checkpoint.get_untracked().ok_or("No checkpoint")?;
        if serde_json::to_vec(&checkpoint).map_err(|e| e.to_string())?
            != serde_json::to_vec(displayed).map_err(|e| e.to_string())?
        {
            return Err("Checkpoint changed; review again".into());
        }
        let docs = Self::docs(tab, checkpoint.snapshot.applied.keys().cloned())?;
        if !self.recovered.get_untracked() {
            let owners = self.checkpoint_owners.get_untracked();
            if docs
                .iter()
                .any(|(p, d)| owners.get(p) != Some(&d.buffer_id.0))
            {
                return Err(
                    "Owned document was closed/reopened; prepare review again"
                        .into(),
                );
            }
        }
        let states = Self::states(&docs);
        let restored =
            checkpoint.restore_staging(&states, self.recovered.get_untracked())?;
        let staged = staged_buffers(&docs, &restored)?;
        let path = self.journal_path()?;
        // Verify persistence before changing buffers during restore as well.
        agent_checkpoint::write(&path, &checkpoint)?;
        // Restore does not save or touch unrelated documents; all conflicts checked first.
        commit_buffers(&docs, staged);
        let mut checkpoint = checkpoint;
        checkpoint.state = State::Restored;
        self.recovered.set(false);
        self.checkpoint_owners.set(
            docs.iter()
                .map(|(p, d)| (p.clone(), d.buffer_id.0))
                .collect(),
        );
        self.checkpoint.set(Some(checkpoint.clone()));
        if let Err(e) = agent_checkpoint::write(&self.journal_path()?, &checkpoint) {
            self.notice.set("Buffers restored, but journal status update failed. No source files saved. Keep this window open and review retained checkpoint.".into());
            return Err(e);
        }
        self.notice
            .set("Checkpoint restored to owned open buffers, not saved.".into());
        Ok(())
    }
}
type StagedBuffers = BTreeMap<
    String,
    (
        lapce_core::buffer::Buffer,
        (
            lapce_xi_rope::Rope,
            lapce_xi_rope::RopeDelta,
            lapce_core::buffer::InvalLines,
        ),
    ),
>;
fn staged_buffers(
    docs: &BTreeMap<String, Rc<Doc>>,
    states: &BTreeMap<String, BufferState>,
) -> Result<StagedBuffers, String> {
    states
        .iter()
        .map(|(path, state)| {
            let mut buffer = docs[path]
                .buffer
                .with_untracked(|b| b.detached_edit_clone());
            let edit = buffer.edit(
                &[(Selection::region(0, buffer.len()), state.text.as_str())],
                EditType::Other,
            );
            if buffer.to_string() != state.text {
                return Err(format!(
                    "Line ending normalization changed reviewed content: {path}"
                ));
            }
            Ok((path.clone(), (buffer, edit)))
        })
        .collect()
}
fn commit_buffers(docs: &BTreeMap<String, Rc<Doc>>, mut staged: StagedBuffers) {
    floem::reactive::batch(|| {
        // Publish every staged owned buffer before notifying dependent views/proxy.
        for (path, (buffer, _)) in &mut staged {
            docs[path]
                .buffer
                .with_untracked(|live| buffer.adopt_revision_clock(live));
        }
        for (path, (buffer, _)) in &staged {
            docs[path].buffer.set(buffer.clone());
        }
        for (path, (_, delta)) in staged {
            docs[&path].apply_deltas(&[delta]);
        }
    });
}
pub fn panel(tab: Rc<WindowTabData>) -> impl View {
    let data = tab.chat.review.clone();
    let prepare = data.clone();
    let prepare_tab = tab.clone();
    let error = data.error;
    let proposals = data.proposal;
    let checkpoints = data.checkpoint;
    let notice = data.notice;
    let cp_data = data.clone();
    let cp_tab = tab.clone();
    // Collapsible: a one-line header by default, opening automatically
    // when a proposal or checkpoint arrives. Prepare stays on the header.
    let open = create_rw_signal(false);
    {
        let proposal = data.proposal;
        let checkpoint = data.checkpoint;
        create_effect(move |_| {
            if proposal.get().is_some() || checkpoint.get().is_some() {
                open.set(true);
            }
        });
    }
    let title_proposal = data.proposal;
    let title_checkpoint = data.checkpoint;
    v_stack((
        h_stack((
            label(move || if open.get() { "\u{25be}" } else { "\u{25b8}" })
                .style(|s| s.width(14.0).selectable(false)),
            label(move || {
                let p = if title_proposal.get().is_some() { 1 } else { 0 };
                let c = if title_checkpoint.get().is_some() { 1 } else { 0 };
                format!("Reviewed edits ({p}/{c})")
            })
            .style(|s| {
                s.font_size(11.0)
                    .flex_grow(1.0)
                    .min_width(0.0)
                    .text_ellipsis()
                    .selectable(false)
            }),
            label(|| "Prepare review".to_string())
                .on_click_stop(move |_| {
                    if let Err(e) = prepare.prepare(&prepare_tab, &prepare_tab.chat.input.get_untracked()) { error.set(e); }
                })
                .style(|s| {
                    s.font_size(11.0)
                        .padding_horiz(8.0)
                        .padding_vert(2.0)
                        .border(1.0)
                        .border_radius(4.0)
                        .cursor(CursorStyle::Pointer)
                }),
        ))
        .style(move |s| s.gap(6.0).padding_vert(2.0).cursor(CursorStyle::Pointer))
        .on_click_stop(move |_| open.update(|o| *o = !*o)),
        container(
        scroll(v_stack((
        label(|| "Paste changes JSON in the composer, then Prepare review. Only existing open files. Checkpoints store private code locally. Act remains disabled.".to_string()).style(|s| s.font_size(11.0).max_width_full()),
        scroll(dyn_stack(move || proposals.get().into_iter(), |p: &Proposal| p.digest().unwrap_or_default(), move |proposal: Proposal| {
            let apply = data.clone(); let discard = data.clone(); let apply_tab = tab.clone();
            let changes = proposal.changes.clone(); let digest = proposal.digest().unwrap_or_default();
            v_stack((
                label(move || format!("Proposal {digest}")).style(|s| s.font_size(10.0).max_width_full()),
                dyn_stack(move || changes.clone(), |c: &Change| c.path.clone(), change_view),
                label(|| "Apply this exact reviewed proposal to buffers".to_string()).on_click_stop(move |_| {
                    if let Err(e) = apply.apply(&apply_tab, &proposal) { apply.error.set(e); }
                }),
                label(|| "Discard proposal".to_string()).on_click_stop(move |_| { discard.proposal.set(None); }),
            ))
        })).style(|s| s.max_height(260.0).width_full()),
        scroll(dyn_stack(move || checkpoints.get().into_iter(), |c: &Checkpoint| format!("{}:{:?}", c.proposal_digest, c.state), move |checkpoint: Checkpoint| {
            let restore = cp_data.clone(); let restore_tab = cp_tab.clone(); let summary = checkpoint.clone();
            let changes: Vec<Change> = checkpoint.snapshot.before.iter().map(|(path, base)| Change {
                path: path.clone(), base: checkpoint.snapshot.applied[path].clone(), proposed: base.text.clone(),
            }).collect();
            v_stack((
                label(move || format!("Checkpoint {:?}: {} file(s). Restart is review-only. Restore fails on later changes.", summary.state, summary.snapshot.applied.len())).style(|s| s.font_size(11.0).max_width_full()),
                dyn_stack(move || changes.clone(), |c: &Change| c.path.clone(), change_view),
                label(|| "Restore this checkpoint (all owned buffers)".to_string()).on_click_stop(move |_| {
                    if let Err(e) = restore.restore(&restore_tab, &checkpoint) { restore.error.set(e); }
                }),
            ))
        })).style(|s| s.max_height(200.0).width_full()),
        label(move || notice.get()).style(|s| s.font_size(11.0).max_width_full()),
        label(move || error.get()).style(|s| s.font_size(11.0).max_width_full()),
    )).style(|s| s.width_full().gap(6.0)))
        .style(|s|s.width_full().max_height(300.0)),
        )
        .style(move |s| s.width_full().apply_if(!open.get(), |s| s.hide())),
    ))
    .style(|s| s.width_full())
}
fn change_view(change: Change) -> impl View {
    let before = change.base.text;
    let after = change.proposed;
    let diff = change_diff(&before, &after);
    v_stack((
        label(move || change.path.clone()),
        scroll(label(move || diff.clone()).style(|s| s.font_size(11.0)))
            .style(|s| s.max_height(140.0).width_full()),
        h_stack((
            scroll(
                label(move || format!("BEFORE\n{before}"))
                    .style(|s| s.font_size(11.0)),
            )
            .style(|s| s.width_pct(50.0).max_height(180.0)),
            scroll(
                label(move || format!("AFTER\n{after}"))
                    .style(|s| s.font_size(11.0)),
            )
            .style(|s| s.width_pct(50.0).max_height(180.0)),
        ))
        .style(|s| s.width_full()),
    ))
    .style(|s| s.width_full())
}

fn change_diff(before: &str, after: &str) -> String {
    let old: Vec<_> = before.split_inclusive('\n').collect();
    let new: Vec<_> = after.split_inclusive('\n').collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let mut result =
        format!("Changed lines at {} (whole changed region)\n", prefix + 1);
    for line in &old[prefix..old.len() - suffix] {
        result.push('-');
        result.push_str(line);
        if !line.ends_with('\n') {
            result.push_str(" [no newline]\n");
        }
    }
    for line in &new[prefix..new.len() - suffix] {
        result.push('+');
        result.push_str(line);
        if !line.ends_with('\n') {
            result.push_str(" [no newline]\n");
        }
    }
    result
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn changed_region_and_final_newline_are_visible() {
        assert_eq!(
            change_diff("same\nbefore\ntail\n", "same\nafter\ntail\n"),
            "Changed lines at 2 (whole changed region)\n-before\n+after\n"
        );
        assert!(change_diff("x", "x\n").contains("-x [no newline]"));
    }
}

/// Strictly parse the proposal JSON the owner pastes or Act mode
/// produces. Pure and total: every rejection carries an actionable
/// message.
fn parse_draft(json: &str) -> Result<Vec<DraftChange>, String> {
    if json.len() > 2 * 1024 * 1024 {
        return Err("Proposal JSON too large".into());
    }
    let draft: Draft = serde_json::from_str(json).map_err(|e| {
        format!(
            "Expected {{\"changes\":[{{\"path\":\"app/User.php\",\"replacement\":\"...\"}}]}}: {e}"
        )
    })?;
    if draft.changes.is_empty() || draft.changes.len() > 32 {
        return Err("Review requires 1-32 existing open files".into());
    }
    Ok(draft.changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_draft_accepts_one_change() {
        let changes = parse_draft(
            r#"{"changes":[{"path":"src/main.rs","replacement":"fn main() {}"}]}"#,
        )
        .unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "src/main.rs");
        assert_eq!(changes[0].replacement, "fn main() {}");
    }

    #[test]
    fn parse_draft_rejects_empty_and_oversized_batches() {
        assert_eq!(
            parse_draft(r#"{"changes":[]}"#).unwrap_err(),
            "Review requires 1-32 existing open files"
        );
        let many = format!(
            "{{\"changes\":[{}]}}",
            vec![
                r#"{"path":"a.rs","replacement":"x"}"#;
                33
            ]
            .join(",")
        );
        assert_eq!(
            parse_draft(&many).unwrap_err(),
            "Review requires 1-32 existing open files"
        );
    }

    #[test]
    fn parse_draft_denies_unknown_fields_and_missing_replacement() {
        assert!(parse_draft(
            r#"{"changes":[{"path":"a.rs","replacement":"x","approved":true}]}"#
        )
        .unwrap_err()
        .contains("Expected"));
        assert!(parse_draft(r#"{"changes":[{"path":"a.rs"}]}"#)
            .unwrap_err()
            .contains("Expected"));
        assert!(parse_draft("not json").is_err());
    }

    #[test]
    fn parse_draft_rejects_over_limit_json() {
        let huge = format!(
            "{{\"changes\":[{{\"path\":\"a.rs\",\"replacement\":\"{}\"}}]}}",
            "x".repeat(2 * 1024 * 1024)
        );
        assert_eq!(
            parse_draft(&huge).unwrap_err(),
            "Proposal JSON too large"
        );
    }
}
