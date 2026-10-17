use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use floem::{
    ext_event::create_ext_action,
    keyboard::Modifiers,
    peniko::kurbo::Rect,
    reactive::{RwSignal, Scope, SignalGet, SignalUpdate, SignalWith},
};
use lapce_core::{
    buffer::rope_text::RopeText, command::FocusCommand, mode::Mode,
    selection::Selection,
};
use lapce_xi_rope::Rope;

use crate::{
    command::{CommandExecuted, CommandKind, InternalCommand, LapceCommand},
    editor::EditorData,
    keypress::{KeyPressFocus, condition::Condition},
    main_split::Editors,
    window_tab::{CommonData, Focus},
};

/// Model prompt budget for the selection being rewritten.
pub const INLINE_EDIT_CODE_CAP: usize = 8_000;
/// Model prompt budget for the surrounding context excerpt.
pub const INLINE_EDIT_CONTEXT_CAP: usize = 4_000;
/// Model prompt budget for the instruction itself.
pub const INLINE_EDIT_INSTRUCTION_CAP: usize = 1_000;

/// Data behind the inline AI edit overlay: a floating instruction input
/// anchored at the selection. On confirm the selection plus the instruction
/// go to the configured chat backend and the reply replaces the selection.
#[derive(Clone, Debug)]
pub struct InlineAiEditData {
    pub active: RwSignal<bool>,
    pub editor: EditorData,
    /// Byte range of the target selection in the target buffer.
    pub start: RwSignal<usize>,
    pub end: RwSignal<usize>,
    /// Buffer revision the range was captured at; the edit only applies
    /// while the target buffer is unchanged.
    pub rev: RwSignal<u64>,
    /// Editor the edit applies to. Rc state: only touched on the UI thread.
    pub target: RwSignal<Option<EditorData>>,
    /// A request is in flight; confirm is a no-op until it lands.
    pub pending: RwSignal<bool>,
    pub cancel: Arc<AtomicBool>,
    pub layout_rect: RwSignal<Rect>,
    pub common: Rc<CommonData>,
}

impl KeyPressFocus for InlineAiEditData {
    fn get_mode(&self) -> Mode {
        Mode::Insert
    }

    fn check_condition(&self, condition: Condition) -> bool {
        matches!(
            condition,
            Condition::InlineAiEditFocus | Condition::ModalFocus
        )
    }

    fn run_command(
        &self,
        command: &LapceCommand,
        count: Option<usize>,
        mods: Modifiers,
    ) -> CommandExecuted {
        match &command.kind {
            CommandKind::Workbench(_) => {}
            CommandKind::Scroll(_) => {}
            CommandKind::Focus(cmd) => {
                self.run_focus_command(cmd);
            }
            CommandKind::Edit(_)
            | CommandKind::Move(_)
            | CommandKind::MultiSelection(_) => {
                self.editor.run_command(command, count, mods);
            }
            CommandKind::MotionMode(_) => {}
        }
        CommandExecuted::Yes
    }

    fn receive_char(&self, c: &str) {
        self.editor.receive_char(c);
    }
}

impl InlineAiEditData {
    pub fn new(cx: Scope, editors: Editors, common: Rc<CommonData>) -> Self {
        let active = cx.create_rw_signal(false);
        let start = cx.create_rw_signal(0);
        let end = cx.create_rw_signal(0);
        let rev = cx.create_rw_signal(0);
        let target = cx.create_rw_signal(None);
        let pending = cx.create_rw_signal(false);
        let layout_rect = cx.create_rw_signal(Rect::ZERO);
        let editor = editors.make_local(cx, common.clone());
        Self {
            active,
            editor,
            start,
            end,
            rev,
            target,
            pending,
            cancel: Arc::new(AtomicBool::new(false)),
            layout_rect,
            common,
        }
    }

    pub fn start(
        &self,
        target: EditorData,
        start: usize,
        end: usize,
        rev: u64,
    ) {
        self.editor.doc().reload(Rope::from(""), true);
        self.editor
            .cursor()
            .update(|cursor| cursor.set_insert(Selection::region(0, 0)));
        self.target.set(Some(target));
        self.start.set(start);
        self.end.set(end);
        self.rev.set(rev);
        self.pending.set(false);
        self.active.set(true);
        self.common.focus.set(Focus::InlineAiEdit);
    }

    fn run_focus_command(&self, cmd: &FocusCommand) -> CommandExecuted {
        match cmd {
            FocusCommand::ModalClose => {
                self.cancel();
            }
            FocusCommand::ConfirmInlineAiEdit => {
                self.confirm();
            }
            _ => return CommandExecuted::No,
        }
        CommandExecuted::Yes
    }

    fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
        self.pending.set(false);
        self.active.set(false);
        if let Focus::InlineAiEdit = self.common.focus.get_untracked() {
            self.common.focus.set(Focus::Workbench);
        }
    }

    /// Send the selection and instruction to the chat backend and replace
    /// the selection with the reply. Skips silently when the backend is
    /// not configured, a request is in flight, or the target buffer moved
    /// on since the overlay opened.
    fn confirm(&self) {
        if self.pending.get_untracked() {
            return;
        }
        let instruction = self
            .editor
            .doc()
            .buffer
            .with_untracked(|buffer| buffer.to_string());
        let instruction = instruction.trim().to_string();
        self.active.set(false);
        if let Focus::InlineAiEdit = self.common.focus.get_untracked() {
            self.common.focus.set(Focus::Workbench);
        }
        if instruction.is_empty() {
            return;
        }
        let chat = self.common.config.get_untracked().ai_chat.clone();
        if !chat.enabled || chat.backend != "ollama" {
            return;
        }
        let Some(target) = self.target.get_untracked() else {
            return;
        };
        let (start, end, rev) = (
            self.start.get_untracked(),
            self.end.get_untracked(),
            self.rev.get_untracked(),
        );
        let doc = target.doc();
        let Some((code, context)) = doc.buffer.with_untracked(|buffer| {
            if buffer.rev() != rev {
                // The user edited the buffer after opening the overlay;
                // the captured range is stale, so do nothing.
                return None;
            }
            let len = buffer.text().len();
            let end = end.min(len);
            let start = start.min(end);
            let code = buffer.slice_to_cow(start..end).to_string();
            // Bounded style context around the selection, aligned to code
            // boundaries so no codepoint is ever split.
            let ctx_start =
                buffer.prev_code_boundary(start.saturating_sub(2_000));
            let ctx_end =
                buffer.next_code_boundary((end + 2_000).min(len));
            let context = buffer.slice_to_cow(ctx_start..ctx_end).to_string();
            Some((code, context))
        }) else {
            return;
        };
        let messages = inline_edit_messages(&code, &instruction, &context);
        self.cancel.store(false, Ordering::Release);
        self.pending.set(true);
        let cancelled = self.cancel.clone();
        let me = self.clone();
        let send = create_ext_action(self.common.scope, move |text: String| {
            me.pending.set(false);
            let text = strip_code_fence(&text);
            if text.is_empty() {
                return;
            }
            let doc = target.doc();
            let unchanged = doc.buffer.with_untracked(|b| b.rev() == rev);
            if !unchanged {
                // The buffer changed while the model was thinking; never
                // stomp on the user's edits.
                return;
            }
            let selection = Selection::region(start, end);
            target.do_edit(&selection, &[(selection.clone(), text.as_str())]);
        });
        std::thread::spawn(move || {
            // Fn(Update) callback: accumulate through a RefCell (mirrors
            // chat_view.rs).
            let text = std::cell::RefCell::new(String::new());
            let _ = crate::chat::ollama(
                &chat.endpoint,
                &chat.model,
                &messages,
                cancelled,
                |update| {
                    if let crate::chat::Update::Delta(delta) = update {
                        text.borrow_mut().push_str(&delta);
                    }
                },
            );
            send(text.into_inner());
        });
    }
}

/// Clip to at most `cap` bytes on a char boundary. Returns the clipped
/// string and whether anything was cut.
fn clip_boundary(s: &str, cap: usize) -> (String, bool) {
    if s.len() <= cap {
        return (s.to_string(), false);
    }
    let mut end = cap;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    (s[..end].to_string(), true)
}

/// Reduce a model reply to just the replacement code: a leading fenced
/// block opener and a closing fence are dropped, then surrounding blank
/// space is trimmed. All slicing happens at `str::find` match offsets and
/// line starts, so multi-byte content is never split.
pub fn strip_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    let body = if let Some(rest) = trimmed.strip_prefix("```") {
        // Drop the opener line (``` or ```rust, any language tag).
        match rest.find('\n') {
            Some(i) => &rest[i + 1..],
            None => "",
        }
    } else {
        trimmed
    };
    // Drop a closing fence only when it is the last non-empty content.
    let body = if body.trim_end().ends_with("```") {
        match body.rfind("```") {
            Some(i) => {
                let line_start = body[..i].rfind('\n').map(|j| j + 1).unwrap_or(0);
                &body[..line_start]
            }
            None => body,
        }
    } else {
        body
    };
    body.trim().to_string()
}

/// Chat messages for one inline edit request. Bounded: the selection,
/// context and instruction are each clipped on a char boundary with an
/// explicit marker, so a huge selection can never blow up the request.
pub fn inline_edit_messages(
    code: &str,
    instruction: &str,
    context: &str,
) -> Vec<lapce_ai::chat::Message> {
    let (code, code_cut) = clip_boundary(code, INLINE_EDIT_CODE_CAP);
    let (context, context_cut) = clip_boundary(context, INLINE_EDIT_CONTEXT_CAP);
    let (instruction, instruction_cut) =
        clip_boundary(instruction, INLINE_EDIT_INSTRUCTION_CAP);

    let mut user = String::with_capacity(
        instruction.len() + code.len() + context.len() + 128,
    );
    user.push_str("Instruction:\n");
    user.push_str(&instruction);
    if instruction_cut {
        user.push_str("\n[instruction truncated to fit the prompt]");
    }
    user.push_str("\n\nCode to replace:\n");
    user.push_str(&code);
    if code_cut {
        user.push_str("\n[code truncated to fit the prompt]");
    }
    if !context.trim().is_empty() {
        user.push_str(
            "\n\nSurrounding code, for style only - do not repeat it:\n",
        );
        user.push_str(&context);
        if context_cut {
            user.push_str("\n[context truncated to fit the prompt]");
        }
    }

    vec![
        lapce_ai::chat::Message {
            role: "system".into(),
            content: "You rewrite code inside an editor. Reply with ONLY \
                the replacement for the given code: no markdown fences, no \
                explanation, no commentary. Match the indentation and style \
                of the surrounding code."
                .into(),
        },
        lapce_ai::chat::Message {
            role: "user".into(),
            content: user,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_code_fence_handles_fenced_and_plain_replies() {
        assert_eq!(strip_code_fence("a + b"), "a + b");
        assert_eq!(strip_code_fence("```rust\na + b\n```"), "a + b");
        assert_eq!(strip_code_fence("```\na + b\n```"), "a + b");
        // No closing fence: the opener is still dropped.
        assert_eq!(strip_code_fence("```rust\na + b"), "a + b");
        // Fence-only reply reduces to empty.
        assert_eq!(strip_code_fence("```rust"), "");
        assert_eq!(strip_code_fence(""), "");
    }

    #[test]
    fn strip_code_fence_is_char_boundary_safe_on_non_ascii() {
        let cases = [
            "```日本語\nlet x = \"grüße 🎉\";\n```",
            "let 日本語 = 1;\n```",
            "```\n// العربية\n",
            "🦀 no fence at all 🦀",
        ];
        for case in cases {
            let out = strip_code_fence(case);
            assert!(std::str::from_utf8(out.as_bytes()).is_ok());
        }
        // A closing fence after non-ASCII code is dropped, code kept.
        let got = strip_code_fence("let 日本語 = 1;\n```");
        assert_eq!(got, "let 日本語 = 1;");
        let got = strip_code_fence("```日本語\nlet x = \"grüße 🎉\";\n```");
        assert_eq!(got, "let x = \"grüße 🎉\";");
    }

    #[test]
    fn clip_boundary_never_splits_a_codepoint() {
        // '日本' is 3 bytes per char; a cap inside a char must pull back.
        let s = "日本語のテキスト";
        let (out, cut) = clip_boundary(s, 4);
        assert!(cut);
        assert_eq!(out, "日");
        let (out, cut) = clip_boundary("abc", 8);
        assert!(!cut);
        assert_eq!(out, "abc");
        let (out, cut) = clip_boundary("abc", 3);
        assert!(!cut);
        assert_eq!(out, "abc");
    }

    #[test]
    fn messages_are_bounded_and_mark_truncation() {
        let code: String = "x".repeat(INLINE_EDIT_CODE_CAP + 500);
        let context: String = "y".repeat(INLINE_EDIT_CONTEXT_CAP + 500);
        let instruction: String = "z".repeat(INLINE_EDIT_INSTRUCTION_CAP + 500);
        let messages = inline_edit_messages(&code, &instruction, &context);
        assert_eq!(messages.len(), 2);
        let user = &messages[1].content;
        assert!(user.contains("[code truncated to fit the prompt]"));
        assert!(user.contains("[context truncated to fit the prompt]"));
        assert!(user.contains("[instruction truncated to fit the prompt]"));
        let budget = INLINE_EDIT_CODE_CAP
            + INLINE_EDIT_CONTEXT_CAP
            + INLINE_EDIT_INSTRUCTION_CAP
            + 256;
        assert!(user.len() <= budget);
    }

    #[test]
    fn messages_keep_non_ascii_intact_at_the_cap() {
        // Cap lands inside multi-byte chars; clipping must not split them.
        let code = format!("{}🦀", "日".repeat(INLINE_EDIT_CODE_CAP));
        let messages = inline_edit_messages(&code, "add a comment", "");
        let user = &messages[1].content;
        assert!(user.contains("[code truncated to fit the prompt]"));
        assert!(std::str::from_utf8(user.as_bytes()).is_ok());
        // Empty context is omitted entirely.
        assert!(!user.contains("Surrounding code"));
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
    }
}
