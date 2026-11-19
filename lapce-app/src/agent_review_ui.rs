//! Owner-only diff review and buffer application. Never parses a chat reply as approval.
use crate::{
    agent_changes::{BufferState, Change, Proposal},
    agent_checkpoint::{self, Checkpoint, State},
    doc::Doc,
    window_tab::WindowTabData,
    config::{LapceConfig, color::LapceColor},
};
use floem::{
    AnyView, IntoView, View,
    reactive::{
        ReadSignal, RwSignal, Scope, SignalGet, SignalUpdate, SignalWith, create_effect,
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
    sync::Arc,
};
#[derive(Clone)]
pub struct ReviewData {
    pub proposal: RwSignal<Option<Proposal>>,
    pub selections: RwSignal<BTreeMap<String, RwSignal<Vec<bool>>>>,
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
    /// Full new file content, or an exact edit via old_text/new_text.
    replacement: Option<String>,
    old_text: Option<String>,
    new_text: Option<String>,
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
            selections: cx.create_rw_signal(BTreeMap::new()),
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
                .map(|c| {
                    let base = current[&c.path].clone();
                    // Exact-edit form: derive the full new content
                    // host-side from the current file (item 228).
                    let proposed_text = match (c.replacement, c.old_text, c.new_text) {
                        (Some(replacement), _, _) => replacement,
                        (None, Some(old_text), Some(new_text)) => {
                            crate::agent_changes::apply_exact_edit(
                                &base.text,
                                &old_text,
                                &new_text,
                            )?
                        }
                        _ => {
                            return Err(
                                "Invalid change shape (parse_draft accepted it)"
                                    .into(),
                            );
                        }
                    };
                    Ok(Change {
                        base,
                        path: c.path.clone(),
                        proposed: docs[&c.path].buffer.with_untracked(|b| {
                            b.line_ending()
                                .normalize(&lapce_xi_rope::Rope::from(
                                    proposed_text.as_str(),
                                ))
                                .to_string()
                        }),
                    })
                })
                .collect::<Result<Vec<Change>, String>>()?,
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
        self.selections.set(proposal.changes.iter().map(|change| {
            let count = crate::agent_changes::hunks(&change.base.text, &change.proposed).len();
            (change.path.clone(), create_rw_signal(vec![true; count]))
        }).collect());
        self.proposal.set(Some(proposal));
        self.run_label.set(String::new());
        self.error.set(String::new());
        self.notice.set(if stale_count == 0 {
            "Review every changed hunk below. Nothing changed yet."
                .to_string()
        } else {
            format!(
                "Review every changed hunk below. Nothing changed yet. {stale_count} file(s) changed since the agent read them - the proposal may clobber those edits."
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
            .filter(|(_, accepted)| *accepted > 0)
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
        checkpoint.snapshot.applied = checkpoint_owned_states(
            &checkpoint.snapshot.before, Self::states(&docs),
        );
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
/// A rejected file remains open but must not re-enter the selected checkpoint.
fn checkpoint_owned_states(
    before: &BTreeMap<String, BufferState>,
    mut current: BTreeMap<String, BufferState>,
) -> BTreeMap<String, BufferState> {
    current.retain(|path, _| before.contains_key(path));
    current
}

type StagedBuffers = crate::buffer_transaction::StagedBuffers<String>;
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
use crate::buffer_transaction::commit_buffers;
/// Counts are from the staged proposal, never inferred from chat prose.
fn proposal_summary(changes: &[Change]) -> String {
    let (added, removed) = changes.iter()
        .flat_map(|change| crate::agent_changes::hunks(&change.base.text, &change.proposed))
        .fold((0, 0), |(added, removed), hunk| (added + hunk.added.len(), removed + hunk.removed.len()));
    let noun = if changes.len() == 1 { "file" } else { "files" };
    format!("{} {noun} proposed +{added} -{removed}", changes.len())
}

pub fn panel(tab: Rc<WindowTabData>) -> impl View {
    let config = tab.common.config;
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
    let selection_data = data.selections;
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
                if let Some(proposal) = title_proposal.get() {
                    proposal_summary(&proposal.changes)
                } else if let Some(checkpoint) = title_checkpoint.get() {
                    format!("Checkpoint {:?}: {} files", checkpoint.state, checkpoint.snapshot.applied.len())
                } else { "No proposed edits".to_string() }
            })
            .style(|s| {
                s.font_size(11.0)
                    .flex_grow(1.0)
                    .flex_basis(0.0)
                    .min_width(0.0)
                    .text_ellipsis()
                    .selectable(false)
            }),
            label(move || if proposals.get().is_some() { "Review".to_string() } else { "Prepare review".to_string() })
                .on_click_stop(move |_| {
                    if proposals.get_untracked().is_some() {
                        open.set(true);
                        return;
                    }
                    let input = prepare_tab.chat.input.get_untracked();
                    error.set(String::new());
                    if !manual_review_candidate(&input) {
                        notice.set("No edit proposal in the composer. A normal Chat answer does not need review. Paste changes JSON here to prepare edits; nothing has changed.".into());
                        open.set(true);
                        return;
                    }
                    notice.set(String::new());
                    if let Err(e) = prepare.prepare(&prepare_tab, &input) { error.set(e); open.set(true); }
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
        .style(move |s| s.width_full().min_width(0.0).gap(6.0).padding_vert(2.0).cursor(CursorStyle::Pointer))
        .on_click_stop(move |_| open.update(|o| *o = !*o)),
        container(
        scroll(v_stack((
        label(|| "Paste changes JSON in the composer, then Prepare review. Existing files or new paths, all under owner review. Checkpoints store private code locally. Act remains disabled.".to_string()).style(|s| s.font_size(11.0).min_width(0.0).max_width_full().text_overflow(floem::style::TextOverflow::Wrap)),
        scroll(dyn_stack(move || proposals.get().into_iter(), |p: &Proposal| p.digest().unwrap_or_default(), move |proposal: Proposal| {
            let apply = data.clone(); let discard = data.clone(); let apply_tab = tab.clone();
            let digest = short_digest(&proposal.digest().unwrap_or_default());
            let selection = selection_data.get_untracked();
            let items: Vec<(Change, RwSignal<Vec<bool>>)> = proposal.changes.iter()
                .filter_map(|c| selection.get(&c.path).map(|mask| (c.clone(), *mask)))
                .collect();
            let items_apply = items.clone();
            let tab_for_preview = tab.clone();
            let accept_all = data.clone(); let accept_tab = tab.clone();
            let all_proposal = proposal.clone();
            let reject_all = data.clone();
            v_stack((
                h_stack((
                    label(|| "Reject all".to_string()).on_click_stop(move |_| {
                        reject_all.proposal.set(None); reject_all.stale.set(Vec::new());
                        reject_all.error.set(String::new());
                        reject_all.notice.set("Proposal rejected. No buffers or source files changed.".into());
                    }).style(move |s| s.font_size(11.0).padding(5.0).border(1.0).border_radius(4.0).cursor(CursorStyle::Pointer).color(config.get().color(LapceColor::SOURCE_CONTROL_REMOVED))),
                    label(|| "Accept all".to_string()).on_click_stop(move |_| {
                        if let Err(e) = accept_all.apply(&accept_tab, &all_proposal) { accept_all.error.set(e); }
                    }).style(move |s| s.font_size(11.0).padding(5.0).border(1.0).border_radius(4.0).cursor(CursorStyle::Pointer).color(config.get().color(LapceColor::SOURCE_CONTROL_ADDED))),
                )).style(|s| s.width_full().min_width(0.0).gap(6.0)),
                label(|| "Accept applies to open buffers only, not saved.".to_string()).style(|s| s.font_size(10.0).width_full().min_width(0.0).text_overflow(floem::style::TextOverflow::Wrap)),
                label(move || format!("Proposal {digest}")).style(|s| s.font_size(10.0).max_width_full()),
                dyn_stack(move || items.clone(), |(c, _): &(Change, RwSignal<Vec<bool>>)| c.path.clone(), move |(change, mask)| change_view(change, Some(mask), config, Some(tab_for_preview.clone())))
                    .style(|s| s.flex_col().width_full().min_width(0.0)),
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
            )).style(|s| s.width_full().min_width(0.0).max_width_full().gap(6.0))
        })).style(|s| s.max_height(260.0).width_full()),
        scroll(dyn_stack(move || checkpoints.get().into_iter(), |c: &Checkpoint| format!("{}:{:?}", c.proposal_digest, c.state), move |checkpoint: Checkpoint| {
            let restore = cp_data.clone(); let restore_tab = cp_tab.clone(); let summary = checkpoint.clone(); let dismiss = cp_data.clone();
            let changes: Vec<Change> = checkpoint.snapshot.before.iter().map(|(path, base)| Change {
                path: path.clone(), base: checkpoint.snapshot.applied[path].clone(), proposed: base.text.clone(),
            }).collect();
            let expanded = create_rw_signal(false);
            let change_count = changes.len();
            // The whole card starts collapsed: a stale
            // checkpoint kept eating the transcript's space
            // after the run ended (host QA on 3562d6a). One
            // click reveals restore/dismiss and the diff
            // toggle.
            let details = create_rw_signal(false);
            v_stack((
                // Keep every row narrow enough for the panel: one long
                // line or one wide row makes the surrounding scroll
                // grow a horizontal bar whose thumb paints over the
                // buttons (host QA on c59c3e9).
                label(move || {
                    if details.get() {
                        format!("Checkpoint {:?}: {} file(s). \u{25be} details", summary.state, summary.snapshot.applied.len())
                    } else {
                        format!("Checkpoint {:?}: {} file(s). \u{25b8} details", summary.state, summary.snapshot.applied.len())
                    }
                })
                .on_click_stop(move |_| details.update(|v| *v = !*v))
                .style(|s| s.font_size(11.0).min_width(0.0).max_width_full().text_overflow(floem::style::TextOverflow::Wrap).cursor(CursorStyle::Pointer).selectable(false)),
                container(v_stack((
                    label(|| "Restart is review-only.".to_string()).style(|s| s.font_size(11.0).min_width(0.0).max_width_full().text_overflow(floem::style::TextOverflow::Wrap)),
                    label(|| "Restore fails on later changes.".to_string()).style(|s| s.font_size(11.0).min_width(0.0).max_width_full().text_overflow(floem::style::TextOverflow::Wrap)),
                    // Diffs start collapsed: an old checkpoint's full
                    // before/after used to shove the whole review section
                    // down the panel (host QA on 253cd71).
                    label(move || {
                        if expanded.get() {
                            "Hide before/after".to_string()
                        } else {
                            format!("Show before/after ({change_count} file(s))")
                        }
                    })
                    .on_click_stop(move |_| expanded.update(|v| *v = !*v))
                    .style(|s| {
                        s.font_size(11.0)
                            .max_width_full()
                            .padding_vert(2.0)
                            .cursor(CursorStyle::Pointer)
                            .selectable(false)
                    }),
                    dyn_stack(
                        move || {
                            if expanded.get() {
                                changes.clone()
                            } else {
                                Vec::new()
                            }
                        },
                        |c: &Change| c.path.clone(),
                        move |change| change_view(change, None, config, None),
                    ),
                    v_stack((
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
                    // Bottom padding keeps Dismiss above any residual
                    // horizontal scrollbar thumb (host QA on f68f497:
                    // the bar painted over the Dismiss hit area).
                    .style(|s| s.gap(4.0).padding_bottom(14.0)),
                ))).style(move |s| if details.get() { s } else { s.hide() }),
            )).style(|s| s.width_full().min_width(0.0).max_width_full().gap(6.0))
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
        label(move || notice.get()).style(|s| s.font_size(11.0).min_width(0.0).max_width_full().text_overflow(floem::style::TextOverflow::Wrap)),
        label(move || error.get()).style(|s| s.font_size(11.0).min_width(0.0).max_width_full().text_overflow(floem::style::TextOverflow::Wrap)),
    )).style(|s| s.width_full().min_width(0.0).max_width_full().gap(6.0)))
        .style(|s|s.width_full().min_width(0.0).max_width_full().max_height(300.0)),
        )
        .style(move |s| s.width_full().min_width(0.0).max_width_full().apply_if(!open.get(), |s| s.hide())),
    ))
    .style(|s| s.width_full().min_width(0.0).max_width_full())
}
/// Review controls in the main editor share the Agent panel's exact hunk mask.
/// Only an exact pair of still-pending snapshots may expose active actions.
pub fn editor_review_toolbar(
    tab: Rc<WindowTabData>, left: Rc<Doc>, right: Rc<Doc>,
) -> impl View {
    let review = tab.chat.review.clone();
    let pending = review.proposal;
    let config = tab.common.config;
    let visible = left.content.with_untracked(|c| c.is_agent_proposal_preview())
        && right.content.with_untracked(|c| c.is_agent_proposal_preview());
    floem::views::dyn_container(move || pending.get(), move |proposal| {
        let path = right.content.with_untracked(|content| match content {
            crate::doc::DocContent::Scratch { name, read_only: true, .. } =>
                name.strip_prefix("Agent proposal proposed (read-only): ").map(str::to_string),
            _ => None,
        });
        let before = left.buffer.with_untracked(|b| b.to_string());
        let after = right.buffer.with_untracked(|b| b.to_string());
        let matched = proposal.as_ref().and_then(|p| preview_change_match(p, path.as_deref(), &before, &after));
        let mask = matched.and_then(|change| review.selections.with_untracked(|m| m.get(&change.path).copied()));
        let body: AnyView = if let (Some(change), Some(mask), Some(proposal)) = (matched, mask, proposal.as_ref()) {
            let counts: Vec<_> = crate::agent_changes::hunks(&change.base.text, &change.proposed).iter().map(|h| (h.added.len(), h.removed.len())).collect();
            let displayed = proposal.clone();
            let apply = review.clone(); let apply_tab = tab.clone();
            h_stack((
                label(|| "Before | Proposed (read-only)".to_string()).style(|s| s.font_size(11.0)),
                label(move || file_selection_summary(&counts, Some(&mask.get()))).style(|s| s.font_size(10.0)),
                label(|| "Accept file".to_string()).on_click_stop(move |_| mask.update(|m| m.fill(true)))
                    .style(move |s| s.font_size(11.0).padding(4.0).border(1.0).border_radius(3.0).cursor(CursorStyle::Pointer).color(config.get().color(LapceColor::SOURCE_CONTROL_ADDED))),
                label(|| "Reject file".to_string()).on_click_stop(move |_| mask.update(|m| m.fill(false)))
                    .style(move |s| s.font_size(11.0).padding(4.0).border(1.0).border_radius(3.0).cursor(CursorStyle::Pointer).color(config.get().color(LapceColor::SOURCE_CONTROL_REMOVED))),
                label(|| "Apply selected".to_string()).on_click_stop(move |_| {
                    let selections = apply.selections.get_untracked();
                    let masks = displayed.changes.iter().map(|c| selections.get(&c.path).map(|m| m.get_untracked()).unwrap_or_default()).collect();
                    if let Err(error) = apply.apply_with(&apply_tab, &displayed, Some(masks)) { apply.error.set(error); }
                }).style(|s| s.font_size(11.0).padding(4.0).border(1.0).border_radius(3.0).cursor(CursorStyle::Pointer)),
            )).style(|s| s.width_full().min_width(0.0).gap(8.0).flex_wrap(floem::style::FlexWrap::Wrap)).into_any()
        } else {
            label(|| "Read-only proposal snapshot. No matching pending review; actions are unavailable.".to_string())
                .style(|s| s.font_size(11.0).width_full().min_width(0.0).text_overflow(floem::style::TextOverflow::Wrap)).into_any()
        };
        body
    }).style(move |s| s.width_full().min_width(0.0).padding(6.0).border_bottom(1.0)
        .border_color(config.get().color(LapceColor::LAPCE_BORDER)).apply_if(!visible, |s| s.hide()))
}

fn preview_change_match<'a>(proposal: &'a Proposal, path: Option<&str>, before: &str, after: &str) -> Option<&'a Change> {
    proposal.changes.iter().find(|change| Some(change.path.as_str()) == path && change.base.text == before && change.proposed == after)
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

fn change_view(change: Change, mask: Option<RwSignal<Vec<bool>>>, config: ReadSignal<Arc<LapceConfig>>, preview_tab: Option<Rc<WindowTabData>>) -> impl View {
    let preview_change = change.clone();
    let can_preview = preview_tab.is_some();
    let before = change.base.text;
    let after = change.proposed;
    let path = change.path;
    let hunks = crate::agent_changes::hunks(&before, &after);
    let file_counts: Vec<(usize, usize)> = hunks.iter().map(|h| (h.added.len(), h.removed.len())).collect();
    let file_status_counts = file_counts.clone();
    let file_mask = mask;

    let diff = change_diff(&before, &after);
    let hunk_rows: Vec<_> = hunks.iter().enumerate().map(|(i, h)| {
        let title = format!("Hunk {} @ line {} (-{}/+{})", i + 1, h.base_start + 1, h.removed.len(), h.added.len());
        let lines: Vec<(usize, bool, String)> = h.removed.iter().map(|line| (false, line))
            .chain(h.added.iter().map(|line| (true, line))).enumerate()
            .map(|(n, (added, line))| (n, added, format!("{}{}{}", if added { "+" } else { "-" }, line.trim_end_matches('\n'), if line.ends_with('\n') { "" } else { " [no newline]" }))).collect();
        (i, title, lines)
    }).collect();
    let body_view: AnyView = match mask {
        Some(mask) => scroll(dyn_stack(
            move || hunk_rows.clone(),
            |(i, _, _)| *i,
            move |(i, title, lines)| {
                v_stack((
                    label(move || format!("{}: {}", title, if mask.with(|m| m.get(i).copied().unwrap_or(true)) { "accepted" } else { "rejected" }))
                        .style(|s| s.font_size(10.0).width_full().min_width(0.0).text_overflow(floem::style::TextOverflow::Wrap)),
                    dyn_stack(move || lines.clone(), |(n, _, _)| *n, move |(_, added, text)| {
                        label(move || text.clone()).style(move |s| s.font_size(11.0).width_full().min_width(0.0)
                            .text_overflow(floem::style::TextOverflow::Wrap)
                            .color(config.get().color(if added { LapceColor::SOURCE_CONTROL_ADDED } else { LapceColor::SOURCE_CONTROL_REMOVED })))
                    }).style(|s| s.flex_col().width_full().min_width(0.0)),
                    h_stack((
                        label(|| "Accept".to_string()).on_click_stop(move |_| mask.update(|m| { if let Some(v) = m.get_mut(i) { *v = true; } }))
                            .style(move |s| s.font_size(10.0).padding_horiz(5.0).padding_vert(2.0).border(1.0).border_radius(3.0).cursor(CursorStyle::Pointer).color(config.get().color(LapceColor::SOURCE_CONTROL_ADDED))),
                        label(|| "Reject".to_string()).on_click_stop(move |_| mask.update(|m| { if let Some(v) = m.get_mut(i) { *v = false; } }))
                            .style(move |s| s.font_size(10.0).padding_horiz(5.0).padding_vert(2.0).border(1.0).border_radius(3.0).cursor(CursorStyle::Pointer).color(config.get().color(LapceColor::SOURCE_CONTROL_REMOVED))),
                    )).style(|s| s.gap(6.0)),
                )).style(|s| s.width_full().min_width(0.0).gap(4.0).padding_bottom(6.0))
            },
        ).style(|s| s.flex_col().width_full().min_width(0.0)))
        .style(|s| s.max_height(180.0).width_full())
        .into_any(),
        None => scroll(
            label(move || diff.clone())
                .style(|s| s.font_size(11.0).min_width(0.0).max_width_full().text_overflow(floem::style::TextOverflow::Wrap)),
        )
            .style(|s| s.max_height(140.0).width_full())
            .into_any(),
    };
    v_stack((
        label(move || review_path_label(&path)).style(|s| s.width_full().min_width(0.0).font_size(11.0).text_overflow(floem::style::TextOverflow::Wrap)),
        label(move || {
            let selected = file_mask.map(|mask| mask.get());
            file_selection_summary(&file_status_counts, selected.as_deref())
        }).style(|s| s.width_full().min_width(0.0).font_size(10.0).text_overflow(floem::style::TextOverflow::Wrap)),
        h_stack((
            label(|| "Accept file".to_string()).on_click_stop(move |_| {
                if let Some(mask) = file_mask { mask.update(|values| values.fill(true)); }
            }).style(move |s| s.font_size(10.0).padding_horiz(5.0).padding_vert(2.0).border(1.0).border_radius(3.0).cursor(CursorStyle::Pointer).color(config.get().color(LapceColor::SOURCE_CONTROL_ADDED))),
            label(|| "Reject file".to_string()).on_click_stop(move |_| {
                if let Some(mask) = file_mask { mask.update(|values| values.fill(false)); }
            }).style(move |s| s.font_size(10.0).padding_horiz(5.0).padding_vert(2.0).border(1.0).border_radius(3.0).cursor(CursorStyle::Pointer).color(config.get().color(LapceColor::SOURCE_CONTROL_REMOVED))),
        )).style(move |s| s.width_full().min_width(0.0).gap(6.0).apply_if(file_mask.is_none(), |s| s.hide())),
        label(|| "Open read-only editor diff".to_string())
            .on_click_stop(move |_| {
                if let Some(tab) = &preview_tab {
                    tab.main_split.open_agent_proposal_diff(&preview_change.path, &preview_change.base.text, &preview_change.proposed);
                }
            }).style(move |s| s.font_size(11.0).cursor(CursorStyle::Pointer).color(config.get().color(LapceColor::EDITOR_LINK)).apply_if(!can_preview, |s| s.hide())),
        body_view,

    ))
    .style(|s| s.width_full().min_width(0.0).max_width_full())
}

/// Give long path components real break points. Label wrapping alone does not
/// constrain intrinsic unbroken text width in nested diff scroll views.
fn review_path_label(path: &str) -> String {
    let mut out = String::new();
    for component in path.split_inclusive('/') {
        if !out.is_empty() { out.push('\n'); }
        for (i, ch) in component.chars().enumerate() {
            if i > 0 && i % 28 == 0 { out.push('\n'); }
            out.push(ch);
        }
    }
    out
}

/// Per-file totals stay proposed; selected totals follow the actual hunk mask.
fn file_selection_summary(counts: &[(usize, usize)], selected: Option<&[bool]>) -> String {
    let total = counts.iter().fold((0, 0), |(a, r), (add, remove)| (a + add, r + remove));
    let Some(selected) = selected else { return format!("Diff +{} -{}", total.0, total.1); };
    let accepted = counts.iter().enumerate().filter(|(i, _)| selected.get(*i).copied().unwrap_or(false))
        .fold((0, 0, 0), |(n, a, r), (_, (add, remove))| (n + 1, a + add, r + remove));
    format!("Proposed +{} -{} | Selected {}/{} hunks +{} -{}", total.0, total.1, accepted.0, counts.len(), accepted.1, accepted.2)
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
    // Small models wrap the proposal in prose or fences; find the
    // JSON object first (item 226).
    let extracted =
        crate::agent_changes::extract_proposal_json(json, None).map_err(|e| {
            format!(
                "Expected a proposal JSON object like {{\"changes\":[{{\"path\":\"relative/file\",\"replacement\":\"...\"}}]}}, but the answer did not parse: {e}"
            )
        })?;
    let draft: Draft = serde_json::from_str(&extracted).map_err(|e| {
        format!(
            "Expected a proposal JSON object like {{\"changes\":[{{\"path\":\"relative/file\",\"replacement\":\"...\"}}]}}, but the answer did not parse: {e}"
        )
    })?;
    if draft.changes.is_empty() || draft.changes.len() > 32 {
        return Err("Review requires 1-32 files".into());
    }
    for c in &draft.changes {
        let full = c.replacement.is_some();
        let old = c.old_text.is_some();
        let new = c.new_text.is_some();
        if full == (old || new) || old != new {
            return Err(
                "Expected a proposal JSON object like {\"changes\":[{\"path\":\"relative/file\",\"replacement\":\"...\"}]} or an exact-edit object like {\"changes\":[{\"path\":\"relative/file\",\"old_text\":\"exact existing text\",\"new_text\":\"its replacement\"}]} - each change needs replacement, or old_text AND new_text, never both forms, never a partial one."
                    .into(),
            );
        }
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
    fn proposal_summary_counts_only_real_staged_diff_lines() {
        let change = Change {
            path: "example.php".into(),
            base: BufferState { revision: 1, text: "old\nkeep\n".into() },
            proposed: "new\nkeep\nextra\n".into(),
        };
        assert_eq!(super::proposal_summary(&[change.clone()]), "1 file proposed +2 -1");
        assert_eq!(super::proposal_summary(&[change.clone(), change]), "2 files proposed +4 -2");
        assert_eq!(super::proposal_summary(&[]), "0 files proposed +0 -0");
    }

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
        assert_eq!(
            changes[0].replacement.as_deref(),
            Some("fn main() {}")
        );
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
    fn parse_draft_accepts_exact_edit_form() {
        let changes = parse_draft(
            r#"{"changes":[{"path":"a.txt","old_text":"dirty","new_text":"clean"}]}"#,
        )
        .unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].replacement.is_none());
        // Mixed or partial forms are refused.
        assert!(parse_draft(
            r#"{"changes":[{"path":"a","replacement":"x","old_text":"y","new_text":"z"}]}"#
        )
        .unwrap_err()
        .contains("Expected"));
        assert!(parse_draft(
            r#"{"changes":[{"path":"a","old_text":"y"}]}"#
        )
        .unwrap_err()
        .contains("Expected"));
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

/// Manual review is optional for ordinary chat text. JSON-looking input
/// still goes through the strict parser, including malformed proposals.
fn manual_review_candidate(input: &str) -> bool {
    let text = input.trim();
    text.starts_with('{') || text.starts_with('[') || text.contains("\"changes\"") || text.contains("```json")
}
#[cfg(test)]
mod manual_review_tests {
    use super::*;
    #[test]
    fn manual_review_plain_chat_is_not_a_parse_failure() {
        for text in ["", "Explain the current file", "The code looks correct. No changes are needed."] {
            assert!(!manual_review_candidate(text));
        }
        for text in ["{broken", "[]", "```json\n{}\n```", "Proposal: {\"changes\":[]}"] {
            assert!(manual_review_candidate(text));
            assert!(parse_draft(text).is_err());
        }
        assert!(manual_review_candidate(r#"{"changes":[{"path":"a.php","replacement":"x"}]}"#));
    }
}

#[cfg(test)]
mod file_selection_summary_tests {
    use super::file_selection_summary;
    #[test]
    fn file_totals_and_selected_lines_are_separate_and_exact() {
        let counts = [(2, 1), (0, 3)];
        assert_eq!(file_selection_summary(&counts, Some(&[true, false])), "Proposed +2 -4 | Selected 1/2 hunks +2 -1");
        assert_eq!(file_selection_summary(&counts, Some(&[false, false])), "Proposed +2 -4 | Selected 0/2 hunks +0 -0");
        assert_eq!(file_selection_summary(&counts, Some(&[true, true])), "Proposed +2 -4 | Selected 2/2 hunks +2 -4");
        assert_eq!(file_selection_summary(&counts, None), "Diff +2 -4");
        assert_eq!(file_selection_summary(&[], Some(&[])), "Proposed +0 -0 | Selected 0/0 hunks +0 -0");
        assert_eq!(file_selection_summary(&counts, Some(&[])), "Proposed +2 -4 | Selected 0/2 hunks +0 -0");
    }
}

#[cfg(test)]
mod review_path_label_tests {
    #[test]
    fn long_paths_have_bounded_unicode_safe_display_lines() {
        let path = format!("src/Services/{}/Example.php", "界".repeat(65));
        let label = super::review_path_label(&path);
        assert!(label.lines().all(|line| line.chars().count() <= 28));
        assert_eq!(label.replace('\n', ""), path);
        assert_eq!(super::review_path_label("example.php"), "example.php");
    }
}

#[cfg(test)]
mod checkpoint_selection_tests {
    use super::*;
    #[test]
    fn rejected_file_does_not_reenter_applied_journal() {
        let state = |text: &str| BufferState { revision: 1, text: text.into() };
        let before = BTreeMap::from([("accepted.php".into(), state("before"))]);
        let current = BTreeMap::from([
            ("accepted.php".into(), state("after")),
            ("rejected.php".into(), state("untouched")),
        ]);
        let applied = checkpoint_owned_states(&before, current);
        assert_eq!(applied.len(), 1);
        assert_eq!(applied["accepted.php"].text, "after");
        let checkpoint = Checkpoint { version: 1, proposal_digest: "a".repeat(64),
            snapshot: crate::agent_changes::Snapshot { before, applied }, state: State::Applied };
        checkpoint.validate().unwrap();
    }
}

#[cfg(test)]
mod editor_review_binding_tests {
    use super::*;
    #[test]
    fn snapshot_controls_require_exact_path_and_both_contents() {
        let proposal = Proposal { run_id: "fixture".into(), changes: vec![Change {
            path: "a.php".into(), base: BufferState { revision: 1, text: "before".into() }, proposed: "after".into(),
        }] };
        assert!(preview_change_match(&proposal, Some("a.php"), "before", "after").is_some());
        for (path, before, after) in [(Some("b.php"), "before", "after"), (None, "before", "after"), (Some("a.php"), "stale", "after"), (Some("a.php"), "before", "stale")] {
            assert!(preview_change_match(&proposal, path, before, after).is_none());
        }
    }
}
