use std::{
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use floem::{
    ext_event::create_ext_action,
    keyboard::Modifiers,
    reactive::{RwSignal, Scope, SignalGet, SignalUpdate, SignalWith},
};
use indexmap::IndexMap;
use lapce_core::{buffer::rope_text::RopeText, mode::Mode};
use lapce_rpc::source_control::{DiffInfo, FileDiff};

use crate::{
    command::{CommandExecuted, CommandKind},
    editor::EditorData,
    keypress::{KeyPressFocus, condition::Condition},
    main_split::Editors,
    window_tab::CommonData,
};

#[derive(Clone, Debug)]
pub struct SourceControlData {
    // VCS modified files & whether they should be included in the next commit
    pub file_diffs: RwSignal<IndexMap<PathBuf, (FileDiff, bool)>>,
    /// Changes staged in the index, shown in their own panel section.
    pub staged_file_diffs: RwSignal<Vec<FileDiff>>,
    pub branch: RwSignal<String>,
    pub branches: RwSignal<im::Vector<String>>,
    pub tags: RwSignal<im::Vector<String>>,
    /// Commits the checked-out branch is ahead of / behind its upstream.
    pub ahead: RwSignal<usize>,
    pub behind: RwSignal<usize>,
    pub editor: EditorData,
    pub common: Rc<CommonData>,
    /// AI commit-message suggestion in flight; clicking again cancels it.
    pub suggesting: RwSignal<bool>,
    pub suggest_cancel: Arc<AtomicBool>,
}

impl KeyPressFocus for SourceControlData {
    fn get_mode(&self) -> Mode {
        Mode::Insert
    }

    fn check_condition(&self, condition: Condition) -> bool {
        matches!(
            condition,
            Condition::PanelFocus | Condition::SourceControlFocus
        )
    }

    fn run_command(
        &self,
        command: &crate::command::LapceCommand,
        count: Option<usize>,
        mods: Modifiers,
    ) -> CommandExecuted {
        match &command.kind {
            CommandKind::Edit(_)
            | CommandKind::Move(_)
            | CommandKind::MultiSelection(_) => {
                self.editor.run_command(command, count, mods)
            }
            _ => CommandExecuted::No,
        }
    }

    fn receive_char(&self, c: &str) {
        self.editor.receive_char(c);
    }
}

impl SourceControlData {
    pub fn new(cx: Scope, editors: Editors, common: Rc<CommonData>) -> Self {
        Self {
            file_diffs: cx.create_rw_signal(IndexMap::new()),
            staged_file_diffs: cx.create_rw_signal(Vec::new()),
            branch: cx.create_rw_signal("".to_string()),
            branches: cx.create_rw_signal(im::Vector::new()),
            tags: cx.create_rw_signal(im::Vector::new()),
            ahead: cx.create_rw_signal(0),
            behind: cx.create_rw_signal(0),
            editor: editors.make_local(cx, common.clone()),
            common,
            suggesting: cx.create_rw_signal(false),
            suggest_cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn commit(&self) {
        let diffs: Vec<FileDiff> = self.file_diffs.with_untracked(|file_diffs| {
            file_diffs
                .iter()
                .filter_map(
                    |(_, (diff, checked))| {
                        if *checked { Some(diff) } else { None }
                    },
                )
                .cloned()
                .collect()
        });
        if diffs.is_empty() {
            return;
        }

        let message = self
            .editor
            .doc()
            .buffer
            .with_untracked(|buffer| buffer.to_string());
        let message = message.trim();
        if message.is_empty() {
            return;
        }

        self.editor.reset();
        self.common.proxy.git_commit(message.to_string(), diffs);
    }

    /// Ask the configured chat backend for a commit message over the
    /// checked files' unified diff. One-shot (not a chat turn); clicking
    /// again while in flight cancels the request.
    pub fn suggest_commit_message(&self) {
        if self.suggesting.get_untracked() {
            self.suggest_cancel.store(true, Ordering::Release);
            return;
        }
        let chat = self.common.config.get_untracked().ai_chat.clone();
        if !chat.enabled || chat.backend != "ollama" {
            return;
        }
        let diffs: Vec<FileDiff> = self.file_diffs.with_untracked(|file_diffs| {
            file_diffs
                .iter()
                .filter_map(
                    |(_, (diff, checked))| {
                        if *checked { Some(diff) } else { None }
                    },
                )
                .cloned()
                .collect()
        });
        if diffs.is_empty() {
            return;
        }
        self.suggest_cancel.store(false, Ordering::Release);
        self.suggesting.set(true);
        let source_control = self.clone();
        // Proxy callbacks must be Send: the proxy callback below captures
        // only the ext action; all Rc state lives in the UI-side closure.
        let send = create_ext_action(
            source_control.common.scope,
            move |diff: Option<String>| {
                let Some(diff) = diff else {
                    source_control.suggesting.set(false);
                    return;
                };
                let messages = commit_message_messages(&diff);
                let cancelled = source_control.suggest_cancel.clone();
                let send = create_ext_action(
                    source_control.common.scope,
                    move |text: String| {
                        source_control.suggesting.set(false);
                        let text = text.trim().to_string();
                        if text.is_empty() {
                            return;
                        }
                        // Replace the whole commit-message buffer with
                        // the suggestion; the user edits or commits as
                        // usual.
                        let editor = &source_control.editor;
                        let doc = editor.doc();
                        let len =
                            doc.buffer.with_untracked(|b| b.text().len());
                        let selection =
                            lapce_core::selection::Selection::region(0, len);
                        editor.do_edit(
                            &selection,
                            &[(selection.clone(), text.as_str())],
                        );
                    },
                );
                std::thread::spawn(move || {
                    // Fn(Update) callback: accumulate through a RefCell
                    // (mirrors chat_view.rs).
                    let text = std::cell::RefCell::new(String::new());
                    let _ = crate::chat::ollama(
                        &chat.endpoint,
                        &chat.model,
                        &messages,
                        cancelled,
                        |update| {
                            if let crate::chat::Update::Delta(delta) = update
                            {
                                text.borrow_mut().push_str(&delta);
                            }
                        },
                    );
                    send(text.into_inner());
                });
            },
        );
        self.common.proxy.git_diff_for_commit(diffs, move |result| {
            let diff = match result {
                Ok(lapce_rpc::proxy::ProxyResponse::GitDiffForCommit {
                    diff,
                }) if !diff.trim().is_empty() => Some(diff),
                _ => None,
            };
            send(diff);
        });
    }
}

/// Prompt for one-shot commit-message generation. Pure and bounded: the
/// proxy already caps the diff at 256 KB; this trims to the model budget
/// on a char boundary and says so when it cuts.
pub fn commit_message_messages(diff: &str) -> Vec<lapce_ai::chat::Message> {
    const PROMPT_DIFF_CAP: usize = 12_000;
    let mut end = PROMPT_DIFF_CAP.min(diff.len());
    while !diff.is_char_boundary(end) {
        end -= 1;
    }
    let mut body = diff[..end].to_string();
    if end < diff.len() {
        body.push_str("\n[diff truncated to fit the prompt]\n");
    }
    vec![
        lapce_ai::chat::Message {
            role: "system".into(),
            content: "You write git commit messages. Reply with one \
                conventional-commit style message: a subject line of at \
                most 72 characters, then a blank line and a short wrapped \
                body only when the change needs explanation. No commentary, \
                no markdown fences."
                .into(),
        },
        lapce_ai::chat::Message {
            role: "user".into(),
            content: format!("Write the commit message for this diff:\n\n{body}"),
        },
    ]
}

/// Merge one DiffInfo into the panel lists: workdir rows keep their
/// per-file commit checkbox state, staged rows replace the staged
/// list wholesale. Extracted from the CoreNotification::DiffInfo
/// handler so the mapping is unit-testable.
pub fn merge_diff_info(
    diff: &DiffInfo,
    previous: &IndexMap<PathBuf, (FileDiff, bool)>,
) -> (IndexMap<PathBuf, (FileDiff, bool)>, Vec<FileDiff>) {
    let diffs = diff
        .diffs
        .iter()
        .cloned()
        .map(|d| {
            let checked = previous.get(d.path()).is_none_or(|(_, c)| *c);
            (d.path().clone(), (d, checked))
        })
        .collect();
    (diffs, diff.staged_diffs.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 0216 panel-model contract, exercised the way the bug
    /// report hit it: a file staged outside the app must land in the
    /// Staged list (and only there), and unstaging must move it back.
    /// The proxy half of this path (git2 index -> DiffInfo) is covered
    /// by dispatch.rs's staged_and_unstaged_are_separate.
    #[test]
    fn merge_diff_info_lists_staged_and_moves_files_between_sections() {
        let staged_path = PathBuf::from("/ws/b.txt");
        let workdir_path = PathBuf::from("/ws/a.txt");
        let diff = DiffInfo {
            diffs: vec![FileDiff::Modified(workdir_path.clone())],
            staged_diffs: vec![FileDiff::Modified(staged_path.clone())],
            ..Default::default()
        };
        let mut previous = IndexMap::new();
        previous.insert(
            workdir_path.clone(),
            (FileDiff::Modified(workdir_path.clone()), false),
        );
        let (diffs, staged) = merge_diff_info(&diff, &previous);
        assert_eq!(staged, vec![FileDiff::Modified(staged_path.clone())]);
        assert!(!diffs.contains_key(&staged_path));
        assert!(diffs.contains_key(&workdir_path));
        // The workdir row keeps its unchecked commit checkbox.
        assert!(!diffs[&workdir_path].1);

        // Unstaging (staged list empties) moves the file back to
        // Changes on the next DiffInfo; new rows default to checked.
        let diff = DiffInfo {
            diffs: vec![
                FileDiff::Modified(workdir_path.clone()),
                FileDiff::Modified(staged_path.clone()),
            ],
            staged_diffs: Vec::new(),
            ..Default::default()
        };
        let (diffs, staged) = merge_diff_info(&diff, &diffs);
        assert!(staged.is_empty());
        assert!(diffs.contains_key(&staged_path));
        assert!(diffs[&staged_path].1);
    }

    #[test]
    fn commit_prompt_shape_and_small_diff() {
        let messages = commit_message_messages("+let a = 1;\n");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
        assert!(messages[1].content.contains("+let a = 1;"));
        assert!(!messages[1].content.contains("truncated"));
        assert!(messages[0].content.contains("72"));
    }

    #[test]
    fn commit_prompt_truncates_on_a_char_boundary() {
        // A multi-byte char straddling the 12000-byte cut must not split.
        let mut diff = "a".repeat(11_999);
        diff.push('🦀'); // bytes 11999..12003 straddle the cap
        diff.push_str("tail");
        let messages = commit_message_messages(&diff);
        let body = &messages[1].content;
        assert!(body.contains("truncated"));
        assert!(!body.contains('🦀'));
        assert!(std::str::from_utf8(body.as_bytes()).is_ok());
        // Under the cap, nothing is cut.
        let messages = commit_message_messages(&diff[..11_000]);
        assert!(!messages[1].content.contains("truncated"));
    }
}
