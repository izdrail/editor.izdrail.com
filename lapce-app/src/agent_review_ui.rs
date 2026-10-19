//! Owner-only diff review and buffer application. Never parses a chat reply as approval.
use crate::{
    agent_changes::{BufferState, Change, Proposal},
    agent_checkpoint::{self, Checkpoint, State},
    doc::Doc,
    window_tab::WindowTabData,
};
use floem::{
    AnyView, IntoView, View,
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
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    rc::Rc,
};
#[derive(Clone)]
pub struct ReviewData {
    pub proposal: RwSignal<Option<Proposal>>,
    pub checkpoint: RwSignal<Option<Checkpoint>>,
    pub recovered: RwSignal<bool>,
    pub error: RwSignal<String>,
    pub notice: RwSignal<String>,
    /// Paths whose live buffer differs from the content the agent
    /// read during its run (item 207): the model derived its
    /// replacement from older content, so those diffs may clobber
    /// edits the owner made while the run was in flight.
    pub stale: RwSignal<Vec<String>>,
    /// Human-readable label for the run behind the current proposal
    /// (mode + request), shown in the walkthrough artifact instead of
    /// the raw run id (item 214). Empty for pasted JSON: no run
    /// context exists.
    pub run_label: RwSignal<String>,
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
            stale: cx.create_rw_signal(Vec::new()),
            run_label: cx.create_rw_signal(String::new()),
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
            #[cfg(not(target_os = "linux"))]
            let exists = true;
            #[cfg(target_os = "linux")]
            let exists = match reader.read(std::path::Path::new(&relative)) {
                Ok(_) => true,
                Err(e) => {
                    // New-file proposal: allowed only as a missing
                    // path under an existing in-root directory.
                    creation_target(&root, &relative)
                        .map_err(|re| format!("{re} ({relative}: {e})"))?;
                    false
                }
            };
            let path = root.join(&relative);
            let existing = all
                .get(&path)
                .cloned()
                .or_else(|| {
                    all.iter().find_map(|(p, d)| {
                        (p.canonicalize().ok().as_ref() == Some(&path))
                            .then(|| d.clone())
                    })
                });
            let doc = match (existing, exists) {
                (Some(doc), _) => doc,
                (None, true) => {
                    return Err(format!("Open file before review: {relative}"));
                }
                (None, false) => tab.main_split.create_unsaved_doc(path.clone()),
            };
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
        self.prepare_with_reads(tab, json, None)
    }
    /// Prepare review, additionally surfacing staleness when `reads`
    /// maps workspace-relative paths to the sha256 hex of the content
    /// the agent actually received for them during the run (item
    /// 207). A mismatch means the model derived its replacement from
    /// older content than the live buffer.
    pub fn prepare_with_reads(
        &self,
        tab: &WindowTabData,
        json: &str,
        reads: Option<&BTreeMap<String, String>>,
    ) -> Result<(), String> {
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
        let stale = match reads {
            Some(reads) => stale_against_reads(&proposal, reads),
            None => Vec::new(),
        };
        let stale_count = stale.len();
        self.stale.set(stale);
        self.owners.set(
            docs.iter()
                .map(|(p, d)| (p.clone(), d.buffer_id.0))
                .collect(),
        );
        self.proposal.set(Some(proposal));
        self.run_label.set(String::new());
        self.error.set(String::new());
        self.notice.set(if stale_count == 0 {
            "Review every before/after file below. Nothing changed yet."
                .to_string()
        } else {
            format!(
                "Review every before/after file below. Nothing changed yet. {stale_count} file(s) changed since the agent read them - the proposal may clobber those edits."
            )
        });
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
        self.apply_with(tab, displayed, None)
    }
    /// masks, when given, are the owner's per-hunk accept toggles -
    /// one bool per hunk per change, recomputed against the pinned
    /// proposal inside stage_with_selection.
    pub fn apply_with(
        &self,
        tab: &WindowTabData,
        displayed: &Proposal,
        masks: Option<Vec<Vec<bool>>>,
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
        // Walkthrough summary: accepted hunk count per change, from the
        // owner's masks when they reviewed hunk by hunk.
        let applied_summary: Vec<(String, usize)> = current
            .changes
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let total =
                    crate::agent_changes::hunks(&c.base.text, &c.proposed)
                        .len();
                let accepted = match &masks {
                    Some(m) => m
                        .get(i)
                        .map(|row| row.iter().filter(|b| **b).count())
                        .unwrap_or(total),
                    None => total,
                };
                (c.path.clone(), accepted)
            })
            .collect();
        let mut snapshot = match &masks {
            Some(masks) => current.stage_with_selection(&reviewed, &states, masks)?,
            None => current.stage(&reviewed, &states)?,
        };
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
        let mut notice = "Applied to open buffers only, not saved. One undo per document; Restore checkpoint reverts all owned buffers if unchanged.".to_string();
        match write_walkthrough_artifact(
            tab,
            &current.run_id,
            &self.run_label.get_untracked(),
            &checkpoint.proposal_digest,
            &applied_summary,
        ) {
            Ok(Some(name)) => {
                tab.chat.plans_version.update(|v| *v += 1);
                notice.push_str(&format!(
                    " Walkthrough saved to .lapce/plans/{name}."
                ));
            }
            Ok(None) => {}
            Err(e) => {
                notice.push_str(&format!(
                    " The walkthrough artifact could not be written: {e}"
                ));
            }
        }
        self.notice.set(notice);
        Ok(())
    }
    /// Drop the current checkpoint and delete its journal file.
    /// Buffers are untouched: the owner gives up the one-click
    /// restore for them. This is how a stale checkpoint from an
    /// older task stops stacking under unrelated proposals (item
    /// 215); the journal is deliberately NOT auto-cleared on
    /// restart, since it is the only recovery path for buffers that
    /// were never saved.
    pub fn dismiss_checkpoint(&self) -> Result<(), String> {
        if let Some(path) = self.path.get_untracked() {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(format!(
                        "Could not remove the checkpoint journal: {e}"
                    ))
                }
            }
        }
        self.path.set(None);
        self.checkpoint.set(None);
        self.checkpoint_owners.set(BTreeMap::new());
        self.recovered.set(false);
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
    let stale = data.stale;
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
        label(|| "Paste changes JSON in the composer, then Prepare review. Existing files or new paths, all under owner review. Checkpoints store private code locally. Act remains disabled.".to_string()).style(|s| s.font_size(11.0).max_width_full()),
        scroll(dyn_stack(move || proposals.get().into_iter(), |p: &Proposal| p.digest().unwrap_or_default(), move |proposal: Proposal| {
            let apply = data.clone(); let discard = data.clone(); let apply_tab = tab.clone();
            let digest = short_digest(&proposal.digest().unwrap_or_default());
            let items: Vec<(Change, RwSignal<Vec<bool>>)> = proposal.changes.iter().map(|c| {
                let count = crate::agent_changes::hunks(&c.base.text, &c.proposed).len();
                (c.clone(), create_rw_signal(vec![true; count]))
            }).collect();
            let items_apply = items.clone();
            v_stack((
                label(move || format!("Proposal {digest}")).style(|s| s.font_size(10.0).max_width_full()),
                dyn_stack(move || items.clone(), |(c, _): &(Change, RwSignal<Vec<bool>>)| c.path.clone(), |(change, mask)| change_view(change, Some(mask))),
                label(|| "Apply accepted hunks".to_string()).on_click_stop(move |_| {
                    let masks: Vec<Vec<bool>> = items_apply.iter().map(|(_, m)| m.get_untracked()).collect();
                    if let Err(e) = apply.apply_with(&apply_tab, &proposal, Some(masks)) { apply.error.set(e); }
                })
                .style(|s| {
                    s.font_size(11.0)
                        .max_width_full()
                        .padding_vert(2.0)
                        .cursor(CursorStyle::Pointer)
                        .selectable(false)
                }),
                label(|| "Discard proposal".to_string()).on_click_stop(move |_| { discard.proposal.set(None); discard.stale.set(Vec::new()); })
                .style(|s| {
                    s.font_size(11.0)
                        .max_width_full()
                        .padding_vert(2.0)
                        .cursor(CursorStyle::Pointer)
                        .selectable(false)
                }),
            )).style(|s| s.width_full().gap(6.0))
        })).style(|s| s.max_height(260.0).width_full()),
        scroll(dyn_stack(move || checkpoints.get().into_iter(), |c: &Checkpoint| format!("{}:{:?}", c.proposal_digest, c.state), move |checkpoint: Checkpoint| {
            let restore = cp_data.clone(); let restore_tab = cp_tab.clone(); let summary = checkpoint.clone(); let dismiss = cp_data.clone();
            let changes: Vec<Change> = checkpoint.snapshot.before.iter().map(|(path, base)| Change {
                path: path.clone(), base: checkpoint.snapshot.applied[path].clone(), proposed: base.text.clone(),
            }).collect();
            v_stack((
                label(move || format!("Checkpoint {:?}: {} file(s). Restart is review-only. Restore fails on later changes.", summary.state, summary.snapshot.applied.len())).style(|s| s.font_size(11.0).max_width_full()),
                dyn_stack(move || changes.clone(), |c: &Change| c.path.clone(), |change| change_view(change, None)),
                h_stack((
                    label(|| "Restore checkpoint (all owned buffers)".to_string()).on_click_stop(move |_| {
                        if let Err(e) = restore.restore(&restore_tab, &checkpoint) { restore.error.set(e); }
                    })
                    .style(|s| {
                        s.font_size(11.0)
                            .max_width_full()
                            .padding_vert(2.0)
                            .cursor(CursorStyle::Pointer)
                            .selectable(false)
                    }),
                    label(|| "Dismiss".to_string()).on_click_stop(move |_| {
                        if let Err(e) = dismiss.dismiss_checkpoint() { dismiss.error.set(e); }
                    })
                    .style(|s| {
                        s.font_size(11.0)
                            .padding_vert(2.0)
                            .cursor(CursorStyle::Pointer)
                            .selectable(false)
                    }),
                ))
                .style(|s| s.gap(12.0)),
            )).style(|s| s.width_full().gap(6.0))
        })).style(|s| s.max_height(200.0).width_full()),
        label(move || {
            let paths = stale.get();
            if paths.is_empty() {
                String::new()
            } else {
                format!(
                    "Changed since the agent read them: {}",
                    paths.join(", ")
                )
            }
        })
        .style(move |s| {
            s.font_size(11.0)
                .max_width_full()
                .apply_if(stale.get().is_empty(), |s| s.hide())
        }),
        label(move || notice.get()).style(|s| s.font_size(11.0).max_width_full()),
        label(move || error.get()).style(|s| s.font_size(11.0).max_width_full()),
    )).style(|s| s.width_full().gap(6.0)))
        .style(|s|s.width_full().max_height(300.0)),
        )
        .style(move |s| s.width_full().apply_if(!open.get(), |s| s.hide())),
    ))
    .style(|s| s.width_full())
}
/// After a reviewed proposal is applied, record what landed as a
/// `.lapce/plans/walkthrough-<epoch>.md` artifact (Antigravity parity:
/// the walkthrough summarizing a completed change). Returns the new
/// file's name; Ok(None) when the window has no workspace root to
/// write into. Errors never fail the apply itself - the buffers and
/// the journal already landed.
fn write_walkthrough_artifact(
    tab: &WindowTabData,
    run_id: &str,
    run_label: &str,
    digest: &str,
    applied: &[(String, usize)],
) -> Result<Option<String>, String> {
    let Some(root) = tab.workspace.path.clone() else {
        return Ok(None);
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir = root.join(".lapce").join("plans");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create the plans directory: {e}"))?;
    let name = format!("walkthrough-{stamp}.md");
    std::fs::write(
        dir.join(&name),
        crate::agent_loop::walkthrough_artifact(
            run_id, run_label, digest, applied,
        ),
    )
    .map_err(|e| format!("Could not write the walkthrough artifact: {e}"))?;
    Ok(Some(name))
}

/// Display form of a proposal digest. The full 64-char hex is
/// unbreakable text: in a narrow panel it overflows its scroll
/// viewport, and the resulting horizontal scrollbar thumb paints
/// over the card's buttons (host QA, item 215). Internal pinning
/// always uses the full digest; only the label is clipped.
pub fn short_digest(digest: &str) -> String {
    let mut short: String = digest.chars().take(16).collect();
    if digest.chars().count() > 16 {
        short.push('\u{2026}');
    }
    short
}

fn change_view(change: Change, mask: Option<RwSignal<Vec<bool>>>) -> impl View {
    let before = change.base.text;
    let after = change.proposed;
    let path = change.path;
    let hunks = crate::agent_changes::hunks(&before, &after);
    let diff = change_diff(&before, &after);
    let hunk_rows: Vec<(usize, String)> = hunks
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let mut body = String::new();
            for line in &h.removed {
                body.push('-');
                body.push_str(line);
                if !line.ends_with('\n') {
                    body.push_str(" [no newline]\n");
                }
            }
            for line in &h.added {
                body.push('+');
                body.push_str(line);
                if !line.ends_with('\n') {
                    body.push_str(" [no newline]\n");
                }
            }
            (
                i,
                format!(
                    "Hunk {} @ line {} (-{}/+{})\n{body}",
                    i + 1,
                    h.base_start + 1,
                    h.removed.len(),
                    h.added.len()
                ),
            )
        })
        .collect();
    let body_view: AnyView = match mask {
        Some(mask) => scroll(dyn_stack(
            move || hunk_rows.clone(),
            |(i, _): &(usize, String)| *i,
            move |(i, text)| {
                h_stack((
                    label(move || {
                        if mask.with(|m| m.get(i).copied().unwrap_or(true)) {
                            "\u{2611}"
                        } else {
                            "\u{2610}"
                        }
                    })
                    .style(|s| s.width(16.0).selectable(false).cursor(CursorStyle::Pointer))
                    .on_click_stop(move |_| {
                        mask.update(|m| {
                            if let Some(v) = m.get_mut(i) {
                                *v = !*v;
                            }
                        })
                    }),
                    label(move || text.clone())
                        .style(|s| s.font_size(11.0).max_width_full()),
                ))
                .style(|s| s.gap(4.0))
            },
        ))
        .style(|s| s.max_height(140.0).width_full())
        .into_any(),
        None => scroll(
            label(move || diff.clone())
                .style(|s| s.font_size(11.0).max_width_full()),
        )
            .style(|s| s.max_height(140.0).width_full())
            .into_any(),
    };
    v_stack((
        label(move || path.clone()).style(|s| s.max_width_full()),
        body_view,
        h_stack((
            scroll(
                label(move || format!("BEFORE\n{before}"))
                    .style(|s| s.font_size(11.0).max_width_full()),
            )
            .style(|s| s.width_pct(50.0).max_height(180.0)),
            scroll(
                label(move || format!("AFTER\n{after}"))
                    .style(|s| s.font_size(11.0).max_width_full()),
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
/// Validate a proposed NEW file: the path must stay under the
/// project root, not exist yet, and sit in an existing directory.
/// Confinement is checked on the lexical path (the file does not
/// exist to canonicalize); parent symlinks are resolved before the
/// root comparison.
fn creation_target(root: &std::path::Path, relative: &str) -> Result<(), String> {
    use std::path::Component;
    let rel = std::path::Path::new(relative);
    if relative.is_empty()
        || rel.is_absolute()
        || !rel
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err("New-file path must stay inside the project".into());
    }
    let root = root
        .canonicalize()
        .map_err(|e| format!("Project root unavailable: {e}"))?;
    let target = root.join(rel);
    if target.exists() {
        return Err("New-file target already exists".into());
    }
    let Some(parent) = target.parent() else {
        return Err("New-file path needs a directory".into());
    };
    let parent = parent
        .canonicalize()
        .map_err(|_| "New-file directory does not exist".to_string())?;
    if !parent.starts_with(&root) {
        return Err("New-file path must stay inside the project".into());
    }
    if rel
        .components()
        .any(|c| matches!(c, Component::Normal(n) if n == ".git"))
    {
        return Err("New-file path must stay inside the project".into());
    }
    Ok(())
}

fn parse_draft(json: &str) -> Result<Vec<DraftChange>, String> {
    if json.len() > 2 * 1024 * 1024 {
        return Err("Proposal JSON too large".into());
    }
    let draft: Draft = serde_json::from_str(json).map_err(|e| {
        format!(
            "Expected a proposal JSON object like {{\"changes\":[{{\"path\":\"relative/file\",\"replacement\":\"...\"}}]}}, but the answer did not parse: {e}"
        )
    })?;
    if draft.changes.is_empty() || draft.changes.len() > 32 {
        return Err("Review requires 1-32 files".into());
    }
    Ok(draft.changes)
}

#[cfg(test)]
mod creation_tests {
    #[test]
    fn creation_target_rules() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join("src").join("old.rs"), "fn old() {}").unwrap();
        std::fs::create_dir(root.join(".git")).unwrap();

        // Missing file under an existing directory: allowed.
        super::creation_target(root, "src/new.rs").unwrap();
        super::creation_target(root, "top.rs").unwrap();
        // Existing file, escape, absolute path, missing directory,
        // .git internals: all refused.
        assert!(super::creation_target(root, "src/old.rs").is_err());
        assert!(super::creation_target(root, "../outside.rs").is_err());
        assert!(super::creation_target(root, "/etc/passwd").is_err());
        assert!(super::creation_target(root, "nope/new.rs").is_err());
        assert!(super::creation_target(root, ".git/hooks/x").is_err());
        assert!(super::creation_target(root, "").is_err());
        // A symlinked parent pointing outside the root is refused.
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            std::os::unix::fs::symlink(outside.path(), root.join("link")).unwrap();
            assert!(super::creation_target(root, "link/new.rs").is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_digest_clips_long_hex_for_narrow_panels() {
        assert_eq!(short_digest("abc123"), "abc123");
        let long = "a".repeat(64);
        assert_eq!(short_digest(&long), format!("{}\u{2026}", "a".repeat(16)));
        // Non-ASCII never panics on a char boundary.
        assert_eq!(short_digest("\u{1f980}\u{1f980}\u{1f980}"), "\u{1f980}\u{1f980}\u{1f980}");
    }

    #[test]
    fn agent_touched_files_reads_walkthrough_artifacts() {
        let dir = std::env::temp_dir().join(format!(
            "lapce-attribution-test-{}",
            std::process::id()
        ));
        let plans = dir.join(".lapce").join("plans");
        std::fs::create_dir_all(&plans).unwrap();
        std::fs::write(
            plans.join("walkthrough-100.md"),
            "# Walkthrough\n\n## Run\n\nr1\n\n## Proposal\n\nd\n\n## Applied changes\n\n- `src/a.rs` (2 hunks)\n- `src/b.rs` (1 hunk)\n\n## Next\n",
        )
        .unwrap();
        std::fs::write(
            plans.join("walkthrough-200.md"),
            "# Walkthrough\n\n## Run\n\nr2\n\n## Proposal\n\nd\n\n## Applied changes\n\n- `src/b.rs` (1 hunk)\n- `src/c.rs` (4 hunks)\n\n## Next\n",
        )
        .unwrap();
        // Decoys: a non-walkthrough file and a no-changes artifact.
        std::fs::write(plans.join("notes.md"), "- `src/decoy.rs` (1 hunk)\n")
            .unwrap();
        std::fs::write(
            plans.join("walkthrough-empty.md"),
            "# Walkthrough\n\n## Applied changes\n\n- (no file changes)\n",
        )
        .unwrap();
        let touched = agent_touched_files(&dir);
        assert_eq!(
            touched.into_iter().collect::<Vec<_>>(),
            vec![
                "src/a.rs".to_string(),
                "src/b.rs".to_string(),
                "src/c.rs".to_string()
            ]
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn agent_touched_files_missing_dir_is_empty() {
        let dir = std::env::temp_dir().join(format!(
            "lapce-attribution-missing-{}",
            std::process::id()
        ));
        assert!(agent_touched_files(&dir).is_empty());
    }

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
            "Review requires 1-32 files"
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
            "Review requires 1-32 files"
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


    fn proposal_of(changes: Vec<(&str, &str)>) -> Proposal {
        Proposal {
            run_id: "run-1".into(),
            changes: changes
                .into_iter()
                .map(|(path, base)| Change {
                    path: path.into(),
                    base: BufferState {
                        revision: 0,
                        text: base.into(),
                    },
                    proposed: "proposed".into(),
                })
                .collect(),
        }
    }

    fn digest_of(text: &str) -> String {
        format!("{:x}", Sha256::digest(text.as_bytes()))
    }

    #[test]
    fn stale_flags_only_paths_that_changed_since_read() {
        let proposal =
            proposal_of(vec![("src/a.rs", "old a"), ("src/b.rs", "same b")]);
        let mut reads = BTreeMap::new();
        // The agent read "old a" but the live base moved on: stale.
        reads.insert("src/a.rs".to_string(), digest_of("older a"));
        // Read and live agree: fresh.
        reads.insert("src/b.rs".to_string(), digest_of("same b"));
        assert_eq!(
            stale_against_reads(&proposal, &reads),
            vec!["src/a.rs".to_string()]
        );
    }

    #[test]
    fn stale_ignores_paths_the_agent_never_read() {
        let proposal = proposal_of(vec![("src/a.rs", "old a")]);
        let reads = BTreeMap::new();
        assert!(stale_against_reads(&proposal, &reads).is_empty());
    }
}

/// Paths whose live base differs from the content digest the agent
/// read during the run: the model derived its replacement from older
/// content, so the owner should review those removals as a possible
/// clobber of edits made while the run was in flight.
pub fn stale_against_reads(
    proposal: &Proposal,
    reads: &BTreeMap<String, String>,
) -> Vec<String> {
    proposal
        .changes
        .iter()
        .filter_map(|c| {
            let seen = reads.get(&c.path)?;
            let live = format!("{:x}", Sha256::digest(c.base.text.as_bytes()));
            (live != *seen).then(|| c.path.clone())
        })
        .collect()
}

/// Relative paths of every file an applied agent run recorded in its
/// walkthrough artifacts (`<workspace>/.lapce/plans/walkthrough-*.md`).
/// The source control panel badges these files so the owner can tell
/// agent-made changes from their own edits in the diff list. Missing
/// or unreadable artifacts are skipped: attribution is advisory.
pub fn agent_touched_files(workspace: &Path) -> BTreeSet<String> {
    let mut touched = BTreeSet::new();
    let dir = workspace.join(".lapce").join("plans");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return touched;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !(name.starts_with("walkthrough-") && name.ends_with(".md")) {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for line in body.lines() {
            // "- `path/inside/worktree.rs` (3 hunks)"
            let Some(rest) = line.strip_prefix("- `") else {
                continue;
            };
            let Some(end) = rest.find('`') else {
                continue;
            };
            touched.insert(rest[..end].to_string());
        }
    }
    touched
}
