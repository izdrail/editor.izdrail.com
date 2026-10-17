use std::{borrow::Cow, ops::Range, path::PathBuf, str::FromStr};

use floem::reactive::{RwSignal, Scope, SignalGet, SignalUpdate, SignalWith, batch};
use lapce_core::{
    buffer::{
        Buffer,
        rope_text::{RopeText, RopeTextRef},
    },
    rope_text_pos::RopeTextPosition,
    selection::Selection,
};
use lsp_types::InsertTextFormat;

use crate::{config::LapceConfig, doc::Doc, editor::EditorData, snippet::Snippet};

// TODO: we could integrate completion lens with this, so it is considered at the same time

/// Redefinition of lsp types inline completion item with offset range
#[derive(Debug, Clone)]
pub struct InlineCompletionItem {
    /// The text to replace the range with.
    pub insert_text: String,
    /// Text used to decide if this inline completion should be shown.
    pub filter_text: Option<String>,
    /// The range (of offsets) to replace  
    pub range: Option<Range<usize>>,
    pub command: Option<lsp_types::Command>,
    pub insert_text_format: Option<InsertTextFormat>,
}
impl InlineCompletionItem {
    pub fn from_lsp(buffer: &Buffer, item: lsp_types::InlineCompletionItem) -> Self {
        let range = item.range.map(|r| {
            let start = buffer.offset_of_position(&r.start);
            let end = buffer.offset_of_position(&r.end);
            start..end
        });
        Self {
            insert_text: item.insert_text,
            filter_text: item.filter_text,
            range,
            command: item.command,
            insert_text_format: item.insert_text_format,
        }
    }

    pub fn apply(
        &self,
        editor: &EditorData,
        start_offset: usize,
    ) -> anyhow::Result<()> {
        let text_format = self
            .insert_text_format
            .unwrap_or(InsertTextFormat::PLAIN_TEXT);

        let selection = if let Some(range) = &self.range {
            Selection::region(range.start, range.end)
        } else {
            Selection::caret(start_offset)
        };

        match text_format {
            InsertTextFormat::PLAIN_TEXT => editor.do_edit(
                &selection,
                &[(selection.clone(), self.insert_text.as_str())],
            ),
            InsertTextFormat::SNIPPET => {
                editor.completion_apply_snippet(
                    &self.insert_text,
                    &selection,
                    Vec::new(),
                    start_offset,
                )?;
            }
            _ => {
                // We don't know how to support this text format
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCompletionStatus {
    /// The inline completion is not active.
    Inactive,
    /// The inline completion is active and is waiting for the server to respond.
    Started,
    /// The inline completion is active and has received a response from the server.
    Active,
}

#[derive(Clone)]
pub struct InlineCompletionData {
    pub status: InlineCompletionStatus,
    /// The active inline completion index in the list of completions.
    pub active: RwSignal<usize>,
    pub items: im::Vector<InlineCompletionItem>,
    pub start_offset: usize,
    pub path: PathBuf,
}
impl InlineCompletionData {
    pub fn new(cx: Scope) -> Self {
        Self {
            status: InlineCompletionStatus::Inactive,
            active: cx.create_rw_signal(0),
            items: im::vector![],
            start_offset: 0,
            path: PathBuf::new(),
        }
    }

    pub fn current_item(&self) -> Option<&InlineCompletionItem> {
        let active = self.active.get_untracked();
        self.items.get(active)
    }

    pub fn next(&mut self) {
        if !self.items.is_empty() {
            let next_index = (self.active.get_untracked() + 1) % self.items.len();
            self.active.set(next_index);
        }
    }

    pub fn previous(&mut self) {
        if !self.items.is_empty() {
            let prev_index = if self.active.get_untracked() == 0 {
                self.items.len() - 1
            } else {
                self.active.get_untracked() - 1
            };
            self.active.set(prev_index);
        }
    }

    pub fn cancel(&mut self) {
        if self.status == InlineCompletionStatus::Inactive {
            return;
        }

        self.items.clear();
        self.status = InlineCompletionStatus::Inactive;
    }

    /// Set the items for the inline completion.  
    /// Sets `active` to `0` and `status` to `InlineCompletionStatus::Active`.
    pub fn set_items(
        &mut self,
        items: im::Vector<InlineCompletionItem>,
        start_offset: usize,
        path: PathBuf,
    ) {
        batch(|| {
            self.items = items;
            self.active.set(0);
            self.status = InlineCompletionStatus::Active;
            self.start_offset = start_offset;
            self.path = path;
        });
    }

    pub fn update_doc(&self, doc: &Doc, offset: usize) {
        if self.status != InlineCompletionStatus::Active {
            doc.clear_inline_completion();
            return;
        }

        if self.items.is_empty() {
            doc.clear_inline_completion();
            return;
        }

        let active = self.active.get_untracked();
        let active = if active >= self.items.len() {
            self.active.set(0);
            0
        } else {
            active
        };

        let item = &self.items[active];
        let text = item.insert_text.clone();

        // TODO: is range really meant to be used for this?
        let offset = item.range.as_ref().map(|r| r.start).unwrap_or(offset);
        let (line, col) = doc
            .buffer
            .with_untracked(|buffer| buffer.offset_to_line_col(offset));
        doc.set_inline_completion(text, line, col);
    }

    pub fn update_inline_completion(
        &self,
        config: &LapceConfig,
        doc: &Doc,
        cursor_offset: usize,
    ) {
        if !config.editor.enable_inline_completion && !config.ai_autocomplete.enabled
        {
            doc.clear_inline_completion();
            return;
        }

        let text = doc.buffer.with_untracked(|buffer| buffer.text().clone());
        let text = RopeTextRef::new(&text);
        let Some(item) = self.current_item() else {
            // TODO(minor): should we cancel completion
            return;
        };

        // Next-edit predictions anchor at their span, away from the
        // trigger offset. The typed-overlap refresh below is only valid
        // for ghost text at the cursor: for an away item it hides the
        // prediction or strips the wrong prefix. Render those exactly
        // like a fresh set instead (full text at the span).
        if anchors_away_from_trigger(item.range.as_ref(), self.start_offset) {
            self.update_doc(doc, cursor_offset);
            return;
        }

        let completion = doc.inline_completion.with_untracked(|cur| {
            let cur = cur.as_deref();
            inline_completion_text(text, self.start_offset, cursor_offset, item, cur)
        });

        match completion {
            ICompletionRes::Hide => {
                doc.clear_inline_completion();
            }
            ICompletionRes::Unchanged => {}
            ICompletionRes::Set(new, shift) => {
                let offset = self.start_offset + shift;
                let (line, col) = text.offset_to_line_col(offset);
                doc.set_inline_completion(new, line, col);
            }
        }
    }
}

enum ICompletionRes {
    Hide,
    Unchanged,
    Set(String, usize),
}

/// Get the text of the inline completion item  
/// A next-edit prediction anchors at its span rather than at the offset
/// the request was triggered from. Pure so the routing in
/// `update_inline_completion` stays testable.
pub fn anchors_away_from_trigger(
    range: Option<&std::ops::Range<usize>>,
    start_offset: usize,
) -> bool {
    range.map(|r| r.start != start_offset).unwrap_or(false)
}

fn inline_completion_text(
    rope_text: impl RopeText,
    start_offset: usize,
    cursor_offset: usize,
    item: &InlineCompletionItem,
    current_completion: Option<&str>,
) -> ICompletionRes {
    let text_format = item
        .insert_text_format
        .unwrap_or(InsertTextFormat::PLAIN_TEXT);

    // TODO: is this check correct? I mostly copied it from completion lens
    let cursor_prev_offset = rope_text.prev_code_boundary(cursor_offset);
    if let Some(range) = &item.range {
        let edit_start = range.start;

        // If the start of the edit isn't where the cursor currently is, and is not at the start of
        // the inline completion, then we ignore it.
        if cursor_prev_offset != edit_start && start_offset != edit_start {
            return ICompletionRes::Hide;
        }
    }

    let text = match text_format {
        InsertTextFormat::PLAIN_TEXT => Cow::Borrowed(&item.insert_text),
        InsertTextFormat::SNIPPET => {
            let Ok(snippet) = Snippet::from_str(&item.insert_text) else {
                return ICompletionRes::Hide;
            };
            let text = snippet.text();

            Cow::Owned(text)
        }
        _ => {
            // We don't know how to support this text format
            return ICompletionRes::Hide;
        }
    };

    let range = start_offset..rope_text.offset_line_end(start_offset, true);
    let prefix = rope_text.slice_to_cow(range);
    // We strip the prefix of the current input from the label.
    // So that, for example `p` with a completion of `println` will show `rintln`.
    let Some(text) = text.strip_prefix(prefix.as_ref()) else {
        return ICompletionRes::Hide;
    };

    if Some(text) == current_completion {
        ICompletionRes::Unchanged
    } else {
        ICompletionRes::Set(text.to_string(), prefix.len())
    }
}

/// Context for deciding whether an automatic (Copilot-style) inline
/// completion request may start. Pure and UI-free so the rules stay testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoTriggerContext {
    /// Master switch: `[ai-autocomplete] enabled`.
    pub ai_enabled: bool,
    /// Opt-in automatic triggering: `[ai-autocomplete] auto-trigger`.
    pub auto_trigger: bool,
    pub insert_mode: bool,
    /// Remote workspaces cannot place the model on the proxy host yet.
    pub is_remote: bool,
    /// The LSP completion popup owns the same visual space and Tab.
    pub completion_popup_open: bool,
    /// Scratch buffers have no file path and are unsupported.
    pub has_file_path: bool,
    pub buffer_loaded: bool,
}
/// Automatic ghost text fires only when every condition holds. Anything the
/// local CPU provider cannot serve correctly falls back to manual Alt+Backslash.
pub fn auto_trigger_allowed(ctx: &AutoTriggerContext) -> bool {
    ctx.ai_enabled
        && ctx.auto_trigger
        && ctx.insert_mode
        && !ctx.is_remote
        && !ctx.completion_popup_open
        && ctx.has_file_path
        && ctx.buffer_loaded
}

/// One recorded edit for the next-edit prompt. Bounded: at most 256 chars
/// per record, truncated on a char boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditRecord {
    pub offset: usize,
    pub text: String,
}

/// Ring of the most recent edits (oldest first), capped at 8 records and
/// 2048 total chars so the prompt stays small no matter how long the
/// session runs.
#[derive(Debug, Default, Clone)]
pub struct EditHistory {
    records: std::collections::VecDeque<EditRecord>,
    total: usize,
}
impl EditHistory {
    const MAX_RECORDS: usize = 8;
    const MAX_TOTAL_CHARS: usize = 2048;
    const MAX_RECORD_CHARS: usize = 256;

    pub fn record(&mut self, offset: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let mut end = Self::MAX_RECORD_CHARS.min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let text = &text[..end];
        self.total += text.len();
        self.records.push_back(EditRecord {
            offset,
            text: text.to_string(),
        });
        while self.records.len() > Self::MAX_RECORDS
            || self.total > Self::MAX_TOTAL_CHARS
        {
            let Some(front) = self.records.pop_front() else {
                break;
            };
            self.total = self.total.saturating_sub(front.text.len());
        }
    }

    pub fn records(&self) -> impl Iterator<Item = &EditRecord> {
        self.records.iter()
    }
}

/// A parsed next-edit prediction. `old` is the exact snippet to replace;
/// empty means insert at the cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextEdit {
    pub old: String,
    pub new: String,
}

/// Parse the model's reply. Tolerant contract: an optional <next-edit>
/// wrapper (and any markdown fences), then <old>...</old> followed by
/// <new>...</new> in that order. Anything else - including a bare NONE -
/// yields no prediction. All slicing happens on ASCII marker boundaries
/// returned by str::find, so non-ASCII content can never split a codepoint.
pub fn parse_next_edit(reply: &str) -> Option<NextEdit> {
    fn inner<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        let start = text.find(&open)? + open.len();
        let end = text[start..].find(&close)? + start;
        Some(&text[start..end])
    }
    fn strip_one_newline(text: &str) -> &str {
        let text = text.strip_prefix('\n').unwrap_or(text);
        text.strip_suffix('\n').unwrap_or(text)
    }
    let old = inner(reply, "old")?;
    let tail_start = reply.find("</old>")? + "</old>".len();
    let new = inner(&reply[tail_start..], "new")?;
    Some(NextEdit {
        old: strip_one_newline(old).to_string(),
        new: strip_one_newline(new).to_string(),
    })
}

/// Build the chat messages for a next-edit request. Pure and bounded: the
/// caller passes the same char-bounded prefix/suffix used for ghost text,
/// and the history ring caps itself.
pub fn next_edit_messages(
    language: &str,
    prefix: &str,
    suffix: &str,
    history: &[EditRecord],
) -> Vec<lapce_ai::chat::Message> {
    let system = "You predict the programmer's next edit in a code editor. \
        Given a file with a <cursor/> marker and a list of recent edits, \
        reply with exactly one edit in this shape and nothing else:
\
        <next-edit>
<old>
exact text from the file to replace (1-10 \
        lines, must appear exactly once)
</old>
<new>
replacement \
        text
</new>
</next-edit>
\
        Use an empty <old></old> for a pure insertion at the cursor. \
        Reply NONE if there is no confident prediction.";
    let mut user = format!("File language: {language}\n");
    if !history.is_empty() {
        user.push_str("Recent edits, oldest first:\n");
        for (i, rec) in history.iter().enumerate() {
            user.push_str(&format!(
                "{}. inserted at offset {}:\n{}\n",
                i + 1,
                rec.offset,
                rec.text
            ));
        }
    }
    user.push_str("---\n");
    user.push_str(prefix);
    user.push_str("<cursor/>");
    user.push_str(suffix);
    vec![
        lapce_ai::chat::Message {
            role: "system".into(),
            content: system.into(),
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
    fn allowed() -> AutoTriggerContext {
        AutoTriggerContext {
            ai_enabled: true,
            auto_trigger: true,
            insert_mode: true,
            is_remote: false,
            completion_popup_open: false,
            has_file_path: true,
            buffer_loaded: true,
        }
    }
    #[test]
    fn auto_trigger_requires_every_guard() {
        assert!(auto_trigger_allowed(&allowed()));
        for ctx in [
            AutoTriggerContext {
                ai_enabled: false,
                ..allowed()
            },
            AutoTriggerContext {
                auto_trigger: false,
                ..allowed()
            },
            AutoTriggerContext {
                insert_mode: false,
                ..allowed()
            },
            AutoTriggerContext {
                is_remote: true,
                ..allowed()
            },
            AutoTriggerContext {
                completion_popup_open: true,
                ..allowed()
            },
            AutoTriggerContext {
                has_file_path: false,
                ..allowed()
            },
            AutoTriggerContext {
                buffer_loaded: false,
                ..allowed()
            },
        ] {
            assert!(!auto_trigger_allowed(&ctx), "{ctx:?}");
        }
    }

    #[test]
    fn history_ring_bounds_and_char_boundaries() {
        let mut history = EditHistory::default();
        // A 300-char record is truncated to 256 on a char boundary.
        let long = "a".repeat(300);
        history.record(10, &long);
        let expected_len = 256;
        assert_eq!(history.records.len(), 1);
        assert_eq!(history.records[0].text.len(), expected_len);
        // Multi-byte chars are never split.
        let mut history = EditHistory::default();
        let unicode = "🦀".repeat(70); // 280 bytes, 70 chars
        history.record(0, &unicode);
        let text = &history.records[0].text;
        assert!(text.len() <= 256);
        assert!(text.chars().all(|c| c == '\u{1F980}'));
        // Ring stays bounded over long sessions.
        let mut history = EditHistory::default();
        for i in 0..50 {
            history.record(i, &"x".repeat(200));
        }
        assert!(history.records.len() <= 8);
        let total: usize = history.records.iter().map(|r| r.text.len()).sum();
        assert!(total <= 2048);
        // Empty inserts are not recorded.
        let mut history = EditHistory::default();
        history.record(0, "");
        assert!(history.records.is_empty());
    }

    #[test]
    fn anchors_away_only_when_the_span_leaves_the_trigger() {
        // Ghost text at the cursor: no range, or a span at the trigger.
        assert!(!anchors_away_from_trigger(None, 42));
        assert!(!anchors_away_from_trigger(Some(&(42..50)), 42));
        assert!(!anchors_away_from_trigger(Some(&(42..42)), 42));
        // Next-edit prediction anchored elsewhere: routes to the span
        // renderer, never the typed-overlap refresh.
        assert!(anchors_away_from_trigger(Some(&(100..110)), 42));
        assert!(anchors_away_from_trigger(Some(&(0..5)), 42));
    }

    #[test]
    fn parse_next_edit_happy_and_none() {
        let reply = "<next-edit>\n<old>\nlet a = 1;\n</old>\n<new>\nlet a = 2;\n</new>\n</next-edit>";
        let expected = NextEdit {
            old: "let a = 1;".into(),
            new: "let a = 2;".into(),
        };
        assert_eq!(parse_next_edit(reply), Some(expected));
        // Fences and prose around the tags are ignored.
        let reply = "Sure!\n```\n<old>x</old><new>y</new>\n```\nDone";
        let expected = NextEdit {
            old: "x".into(),
            new: "y".into(),
        };
        assert_eq!(parse_next_edit(reply), Some(expected));
        // NONE, empty, and tag-less replies yield nothing.
        assert_eq!(parse_next_edit("NONE"), None);
        assert_eq!(parse_next_edit(""), None);
        assert_eq!(parse_next_edit("no prediction today"), None);
        // <new> before <old> violates the order contract.
        assert_eq!(parse_next_edit("<new>y</new><old>x</old>"), None);
        // Empty old is a pure insertion at the cursor.
        let expected = NextEdit {
            old: String::new(),
            new: "inserted".into(),
        };
        assert_eq!(parse_next_edit("<old></old><new>inserted</new>"), Some(expected));
    }

    #[test]
    fn parse_next_edit_non_ascii() {
        let reply = "<old>\nlet 名 = \"café 🦀\";\n</old>\n<new>\nlet 名 = \"café ☕\";\n</new>";
        let edit = parse_next_edit(reply).unwrap();
        assert_eq!(edit.old, "let 名 = \"café 🦀\";");
        assert_eq!(edit.new, "let 名 = \"café ☕\";");
        // Combining marks and RTL text pass through byte-for-byte.
        let reply = "<old>e\u{301} العربية</old><new>日本語</new>";
        let edit = parse_next_edit(reply).unwrap();
        let expected_old = "e\u{301} العربية";
        assert_eq!(edit.old, expected_old);
        assert_eq!(edit.new, "日本語");
    }

    #[test]
    fn next_edit_messages_shape_and_bounds() {
        let history = vec![
            EditRecord {
                offset: 42,
                text: "let a = 1;".into(),
            },
            EditRecord {
                offset: 99,
                text: "🦀".into(),
            },
        ];
        let messages = next_edit_messages("rust", "fn a() {", "}", &history);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
        assert!(messages[1].content.contains("<cursor/>"));
        assert!(messages[1].content.contains("let a = 1;"));
        assert!(messages[1].content.contains("🦀"));
        assert!(messages[0].content.contains("<next-edit>"));
        // A huge context stays bounded by the caller's slice, and the
        // builder adds only a small constant.
        let big = "x".repeat(24_000);
        let messages = next_edit_messages("php", &big, "", &[]);
        assert!(messages[1].content.len() < 24_000 + 1_000);
    }
}
