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

#[test]
fn all_pair_insert_skip_and_selection_wrap_use_native_action() {
    for (open, close) in [('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\''), ('`', '`')] {
        let mut buffer = Buffer::new("");
        let mut cursor = Cursor::new(CursorMode::Insert(Selection::caret(0)), None, None);
        Action::insert(&mut cursor, &mut buffer, &open.to_string(), &|_,_,_|None, true, true);
        assert_eq!(buffer.to_string(), format!("{open}{close}"));
        assert_eq!(cursor.offset(), 1);
        Action::insert(&mut cursor, &mut buffer, &close.to_string(), &|_,_,_|None, true, true);
        assert_eq!(buffer.to_string(), format!("{open}{close}"));
        assert_eq!(cursor.offset(), 2);
        let mut buffer = Buffer::new("value");
        let mut cursor = Cursor::new(CursorMode::Insert(Selection::region(0, 5)), None, None);
        Action::insert(&mut cursor, &mut buffer, &open.to_string(), &|_,_,_|None, true, true);
        assert_eq!(buffer.to_string(), format!("{open}value{close}"));
        buffer.do_undo().unwrap();
        assert_eq!(buffer.to_string(), "value");
        buffer.do_redo().unwrap();
        assert_eq!(buffer.to_string(), format!("{open}value{close}"));
    }
}
#[test]
fn nested_insertions_and_paste_are_not_reinterpreted_as_pairs() {
    let mut buffer = Buffer::new("");
    let mut cursor = Cursor::new(CursorMode::Insert(Selection::caret(0)), None, None);
    for text in ["(", "[", "{"] {
        Action::insert(&mut cursor, &mut buffer, text, &|_,_,_|None, true, true);
    }
    assert_eq!(buffer.to_string(), "([{}])");
    assert_eq!(cursor.offset(), 3);
    Action::insert(&mut cursor, &mut buffer, "pasted(\"text\")", &|_,_,_|None, true, true);
    assert_eq!(buffer.to_string(), "([{pasted(\"text\")}])");
}
#[test]
fn smart_enter_expands_all_structural_pairs_with_configured_indent_and_one_undo() {
    use crate::indent::IndentStyle;
    for pair in ["{}", "[]", "()"] {
        for style in [IndentStyle::Tabs, IndentStyle::Spaces(2), IndentStyle::Spaces(4)] {
            let mut buffer = Buffer::new(pair);
            buffer.set_indent_style(style);
            let mut cursor = Cursor::new(CursorMode::Insert(Selection::caret(1)), None, None);
            Action::do_edit(&mut cursor, &mut buffer, &EditCommand::InsertNewLine, &mut NoClipboard,
                &mut Register::default(), EditConf { comment_tokens: &|_| ("", ""), modal: false,
                smart_tab: true, keep_indent: true, auto_indent: true });
            assert_eq!(buffer.to_string(), format!("{}\n{}\n{}", &pair[..1], style.as_str(), &pair[1..]));
            assert_eq!(cursor.offset(), 2 + style.as_str().len());
            buffer.do_undo().unwrap();
            assert_eq!(buffer.to_string(), pair);
            buffer.do_redo().unwrap();
            assert_eq!(buffer.to_string(), format!("{}\n{}\n{}", &pair[..1], style.as_str(), &pair[1..]));
        }
    }
}
#[test]
fn indentation_backspace_obeys_tab_and_space_stops() {
    use crate::indent::IndentStyle;
    for (text, style, expected) in [("      x", IndentStyle::Spaces(4), "    x"),
        ("    x", IndentStyle::Spaces(4), "x"), ("\t\tx", IndentStyle::Tabs, "\tx")] {
        let offset = text.find('x').unwrap();
        let mut buffer = Buffer::new(text);
        buffer.set_indent_style(style);
        let mut cursor = Cursor::new(CursorMode::Insert(Selection::caret(offset)), None, None);
        backspace(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), expected);
    }
}

#[test]
fn smart_enter_multiple_carets_keeps_each_inner_offset_and_one_history_step() {
    use crate::{indent::IndentStyle, line_ending::LineEnding};
    for (ending, newline) in [(LineEnding::Lf, "\n"), (LineEnding::CrLf, "\r\n")] {
        let original = format!("{{}}{newline}[]");
        let mut buffer = Buffer::new(&original);
        buffer.set_line_ending(ending);
        buffer.set_indent_style(IndentStyle::Spaces(2));
        let mut selection = Selection::caret(1);
        selection.add_region(SelRegion::caret(3 + newline.len()));
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);
        Action::do_edit(&mut cursor, &mut buffer, &EditCommand::InsertNewLine, &mut NoClipboard,
            &mut Register::default(), EditConf { comment_tokens: &|_| ("", ""), modal: false,
            smart_tab: true, keep_indent: true, auto_indent: true });
        let expected = format!("{{{newline}  {newline}}}{newline}[{newline}  {newline}]");
        assert_eq!(buffer.to_string(), expected);
        let first = 1 + newline.len() + 2;
        let second = first + newline.len() + 1 + newline.len() + 1 + newline.len() + 2;
        assert_eq!(cursor.edit_selection(&buffer).regions().iter().map(|r|r.end).collect::<Vec<_>>(), [first, second]);
        buffer.do_undo().unwrap();
        assert_eq!(buffer.to_string(), original);
        buffer.do_redo().unwrap();
        assert_eq!(buffer.to_string(), expected);
    }
}
#[test]
fn separate_enter_presses_remain_separate_undo_steps() {
    let mut buffer = Buffer::new("{}");
    let mut cursor = Cursor::new(CursorMode::Insert(Selection::caret(1)), None, None);
    for _ in 0..2 {
        Action::do_edit(&mut cursor, &mut buffer, &EditCommand::InsertNewLine, &mut NoClipboard,
            &mut Register::default(), EditConf { comment_tokens: &|_| ("", ""), modal: false,
            smart_tab: true, keep_indent: true, auto_indent: true });
    }
    buffer.do_undo().unwrap();
    assert_eq!(buffer.to_string(), "{\n    \n}");
    buffer.do_undo().unwrap();
    assert_eq!(buffer.to_string(), "{}");
}

#[test]
fn independent_pairing_controls_cover_every_pair() {
    use crate::editor::PairingOptions;
    for (opening, closing, quote) in [('(', ')', false), ('[', ']', false), ('{', '}', false), ('"', '"', true), ('\'', '\'', true), ('`', '`', true)] {
        for enabled in [false, true] {
            let mut buffer = Buffer::new("");
            let mut cursor = Cursor::new(CursorMode::Insert(Selection::caret(0)), None, None);
            Action::insert_with_pairing_options(&mut cursor, &mut buffer, &opening.to_string(), &|_,_,_| None,
                PairingOptions { brackets: !quote && enabled, quotes: quote && enabled, skip_closing: false, surround: false });
            assert_eq!(buffer.to_string(), if enabled {format!("{opening}{closing}")} else {opening.to_string()});
            assert_eq!(cursor.offset(), 1);
        }
        for skip in [false, true] {
            let mut buffer = Buffer::new(&closing.to_string());
            let mut cursor = Cursor::new(CursorMode::Insert(Selection::caret(0)), None, None);
            Action::insert_with_pairing_options(&mut cursor, &mut buffer, &closing.to_string(), &|_,_,_| None,
                PairingOptions { brackets: false, quotes: false, skip_closing: skip, surround: false });
            assert_eq!(buffer.to_string(), if skip {closing.to_string()} else {format!("{closing}{closing}")});
            assert_eq!(cursor.offset(), 1);
        }
    }
}

#[test]
fn enter_controls_separate_structural_expansion_from_indent_retention() {
    for (keep, smart, expected) in [(true, true, "  {\n      \n  }"), (true, false, "  {\n  }"), (false, false, "  {\n}")] {
        let mut buffer = Buffer::new("  {}");
        buffer.set_indent_style(crate::indent::IndentStyle::Spaces(4));
        let mut cursor = Cursor::new(CursorMode::Insert(Selection::caret(3)), None, None);
        Action::do_edit(&mut cursor, &mut buffer, &EditCommand::InsertNewLine, &mut NoClipboard,
            &mut Register::default(), EditConf { comment_tokens: &|_| ("", ""), modal: false,
                smart_tab: true, keep_indent: keep, auto_indent: smart });
        assert_eq!(buffer.to_string(), expected, "keep={keep}, smart={smart}");
        buffer.do_undo().unwrap();
        assert_eq!(buffer.to_string(), "  {}");
        buffer.do_redo().unwrap();
        assert_eq!(buffer.to_string(), expected);
    }
}

#[test]
fn escaped_quotes_insert_literally_instead_of_skipping_or_pairing() {
    for quote in ['"', '\'', '`'] {
        for slashes in [1, 2, 3, 4] {
            for existing in [false, true] {
                let prefix = "\\".repeat(slashes);
                let text = format!("{prefix}{}", if existing {quote.to_string()} else {String::new()});
                let mut buffer = Buffer::new(&text);
                let mut cursor = Cursor::new(CursorMode::Insert(Selection::caret(slashes)), None, None);
                Action::insert(&mut cursor, &mut buffer, &quote.to_string(), &|_,_,_| None, true, true);
                let expected = if slashes % 2 == 1 {
                    format!("{prefix}{quote}{}", if existing {quote.to_string()} else {String::new()})
                } else if existing { format!("{prefix}{quote}") }
                else { format!("{prefix}{quote}{quote}") };
                assert_eq!(buffer.to_string(), expected, "slashes={slashes}, existing={existing}");
                assert_eq!(cursor.offset(), slashes + 1);
            }
        }
    }
}

#[test]
fn explicit_selection_with_surround_disabled_replaces_instead_of_skipping() {
    for closing in [')', ']', '}', '"', '\'', '`'] {
        let text = format!("{closing}abc");
        let mut buffer = Buffer::new(&text);
        let mut cursor = Cursor::new(CursorMode::Insert(Selection::region(text.len(), 0)), None, None);
        Action::insert(&mut cursor, &mut buffer, &closing.to_string(), &|_,_,_| None, true, false);
        assert_eq!(buffer.to_string(), closing.to_string());
        buffer.do_undo().unwrap();
        assert_eq!(buffer.to_string(), text);
    }
}

#[test]
fn partner_filter_is_per_caret_and_does_not_suppress_literal_edits() {
    use crate::editor::PairingOptions;
    let mut buffer = Buffer::new("\n");
    let mut selection = Selection::caret(0);
    selection.add_region(SelRegion::caret(1));
    let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);
    Action::insert_with_partner_filter(&mut cursor, &mut buffer, "(", &|_,_,_| None,
        PairingOptions::legacy(true, true), &|offset| offset == 0);
    assert_eq!(buffer.to_string(), "()\n(");
    buffer.do_undo().unwrap();
    assert_eq!(buffer.to_string(), "\n");
    buffer.do_redo().unwrap();
    assert_eq!(buffer.to_string(), "()\n(");
}

#[test]
fn partner_filter_preserves_closer_skip_and_selection_wrap() {
    use crate::editor::PairingOptions;
    let mut buffer = Buffer::new(")");
    let mut cursor = Cursor::new(CursorMode::Insert(Selection::caret(0)), None, None);
    Action::insert_with_partner_filter(&mut cursor, &mut buffer, ")", &|_,_,_| None,
        PairingOptions::legacy(true, true), &|_| false);
    assert_eq!(buffer.to_string(), ")");
    assert_eq!(cursor.offset(), 1);
    let mut cursor = Cursor::new(CursorMode::Insert(Selection::region(0, 1)), None, None);
    Action::insert_with_partner_filter(&mut cursor, &mut buffer, "(", &|_,_,_| None,
        PairingOptions::legacy(true, true), &|_| false);
    assert_eq!(buffer.to_string(), "())");
}
