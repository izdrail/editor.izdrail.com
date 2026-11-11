#![allow(clippy::manual_clamp)]

pub mod diagnostics;
pub mod directory;
pub mod encoding;
pub mod language;
pub mod lens;
pub mod meta;
pub mod rope_text_pos;
pub mod style;
pub mod syntax;
// This is primarily being re-exported to avoid changing every single usage
// in lapce-app. We should probably remove this at some point.
pub use floem_editor_core::*;

pub mod lexilla;
pub mod textmate;

pub mod folding;

pub mod project_profile;

pub mod web_metadata;

pub mod web_templates;

#[cfg(test)]
mod line_ending_save_tests {
    use crate::{buffer::{Buffer, rope_text::RopeText}, editor::EditType, line_ending::LineEnding, selection::Selection};
    use lapce_xi_rope::Rope;

    #[test]
    fn already_normalized_save_does_not_consume_undo() {
        for (ending, newline) in [(LineEnding::Lf, "\n"), (LineEnding::CrLf, "\r\n")] {
            let original = format!("<p>text</p>{newline}");
            assert!(ending.normalize_delta(&Rope::from(original.as_str())).is_none());
            let mut buffer = Buffer::new(original.as_str());
            buffer.set_line_ending(ending);
            buffer.edit(&[(Selection::region(1, 2), "article"), (Selection::region(9, 10), "article")], EditType::Completion);
            assert!(buffer.normalize_line_endings().is_none());
            buffer.do_undo().expect("one rename to undo");
            assert_eq!(buffer.text().to_string(), original);
        }
    }

    #[test]
    fn changed_line_endings_still_produce_normalization_deltas() {
        for (ending, text, expected) in [
            (LineEnding::Lf, "a\r\nb\rc\n", "a\nb\nc\n"),
            (LineEnding::CrLf, "a\nb\rc\r\n", "a\r\nb\r\nc\r\n"),
        ] {
            let text = Rope::from(text);
            let delta = ending.normalize_delta(&text).expect("conversion needed");
            assert_eq!(delta.apply(&text).to_string(), expected);
        }
        assert!(LineEnding::Lf.normalize_delta(&Rope::from("")).is_none());
        assert!(LineEnding::CrLf.normalize_delta(&Rope::from("no newline")).is_none());
    }
}
