//! Regression tests exercise the vendored action used by ordinary documents.
use crate::{
    buffer::{Buffer, rope_text::RopeText},
    command::EditCommand,
    cursor::{Cursor, CursorMode},
    editor::{Action, EditConf},
    register::{Clipboard, Register},
    selection::{SelRegion, Selection},
};
struct NoClipboard;
impl Clipboard for NoClipboard {
    fn get_string(&mut self) -> Option<String> { None }
    fn put_string(&mut self, _: impl AsRef<str>) { panic!("unexpected clipboard write"); }
}
fn backspace(buffer: &mut Buffer, cursor: &mut Cursor) {
    Action::do_edit(cursor, buffer, &EditCommand::DeleteBackward, &mut NoClipboard,
        &mut Register::default(), EditConf { comment_tokens: &|_| ("", ""),
        modal: false, smart_tab: true, keep_indent: true, auto_indent: true });
}
#[test]
fn every_empty_pair_deletes_at_multiple_carets_and_undo_redoes_together() {
    for pair in ["()", "[]", "{}", "\"\"", "''", "``"] {
        let original = format!("{pair}\n{pair}");
        let mut buffer = Buffer::new(&original);
        let mut selection = Selection::caret(1);
        selection.add_region(SelRegion::caret(4));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);
        backspace(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "\n", "pair {pair}");
        assert_eq!(cursor.edit_selection(&buffer).regions().iter().map(|r|r.end).collect::<Vec<_>>(), [0, 1]);
        buffer.do_undo().expect("one undo restores both pairs");
        assert_eq!(buffer.to_string(), original);
        buffer.do_redo().expect("one redo removes both pairs");
        assert_eq!(buffer.to_string(), "\n");
    }
}
#[test]
fn explicit_selection_does_not_expand_to_a_closer() {
    for (text, expected) in [("()", ")"), ("(  )", "  )"), ("\"\"", "\"")] {
        let mut buffer = Buffer::new(text);
        let mut cursor = Cursor::new(CursorMode::Insert(Selection::region(0, 1)), None, None);
        backspace(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), expected);
    }
}
#[test]
fn pair_deletion_is_adjacent_and_does_not_eat_whitespace_or_content() {
    for text in ["(  )", "(x)", "(]", "(\n)"] {
        let mut buffer = Buffer::new(text);
        let mut cursor = Cursor::new(CursorMode::Insert(Selection::caret(1)), None, None);
        backspace(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), &text[1..]);
    }
}
#[test]
fn mixed_carets_ranges_and_unicode_keep_independent_edits() {
    let mut buffer = Buffer::new("()\nabc\né[]");
    let mut selection = Selection::caret(1);
    selection.add_region(SelRegion::new(3, 5, None));
    selection.add_region(SelRegion::caret(10));
    let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);
    backspace(&mut buffer, &mut cursor);
    assert_eq!(buffer.to_string(), "\nc\né");
    buffer.do_undo().unwrap();
    assert_eq!(buffer.to_string(), "()\nabc\né[]");
}
