//! Pair insertion context from the existing lexer snapshot, never a second parser.
use lexilla_rs::Snapshot;

/// An async snapshot may add partners only for the exact current revision
/// and byte length. Unknown context always leaves literal insertion intact.
pub fn permits_current_partner(snapshot: Option<&Snapshot>, snapshot_revision: u64,
    buffer_revision: u64, buffer_len: usize, offset: usize) -> bool {
    snapshot_revision == buffer_revision && snapshot.is_some_and(|snapshot| {
        snapshot.len() == buffer_len && permits_partner(snapshot, offset)
    })
}

/// This policy only decides whether to ADD a partner. Literal insertion,
/// explicit wrapping and moving over existing closers are separate actions.
/// A snapshot must belong to the current text, checked by the caller.
pub fn permits_partner(snapshot: &Snapshot, offset: usize) -> bool {
    if offset > snapshot.len() { return false; }
    let scope = |offset: usize| snapshot.styles().get(offset).and_then(|style| snapshot.scope(*style));
    let previous = offset.checked_sub(1).and_then(scope);
    let current = scope(offset);
    let comment = |scope: Option<&str>| scope.is_some_and(|s| s.starts_with("comment"));
    let string = |scope: Option<&str>| scope.is_some_and(|s| s.starts_with("string"));
    if comment(current) || comment(previous) { return false; }
    !(string(previous) && (string(current) || offset == snapshot.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{language::LapceLanguage, lexilla};
    use lapce_xi_rope::Rope;

    #[test]
    fn pairing_context_existing_lexers_distinguish_code_strings_and_comments() {
        use LapceLanguage::*;
        for (language, text, string_marker, comment_marker) in [
            (Php, "<?php\n$x = \"a value\"; // note here\n", "value", "here"),
            (Python, "value = \"a value\" # note here\n", "value\"", "here"),
            (Rust, "let value = \"a value\"; // note here\n", "value\"", "here"),
            (Javascript, "const value = \"a value\"; // note here\n", "value\"", "here"),
            (Typescript, "const value: string = \"a value\"; // note here\n", "value\"", "here"),
            (Css, "a { content: \"a value\"; /* note here */ }\n", "value\"", "here"),
            (Html, "<a title=\"a value\">x</a><!-- note here -->\n", "value\"", "here"),
            (Blade, "<a title=\"a value\">x</a>{{-- note here --}}\n", "value\"", "here"),
            (Vue, "<template><a title=\"a value\">x</a><!-- note here --></template>\n", "value\"", "here"),
        ] {
            let snapshot = lexilla::highlight(language, &Rope::from(text)).unwrap_or_else(|| panic!("missing lexer: {language:?}"));
            assert!(!permits_partner(&snapshot, text.rfind(string_marker).unwrap() + 2), "{language:?}: string");
            assert!(!permits_partner(&snapshot, text.find(comment_marker).unwrap() + 1), "{language:?}: comment");
            assert!(permits_partner(&snapshot, 0), "{language:?}: code start");
            assert!(!permits_partner(&snapshot, text.len()+1));
        }
    }

    #[test]
    fn pairing_context_line_comment_end_precedes_unstyled_newline() {
        for (language, text) in [
            (crate::language::LapceLanguage::Php, "<?php\n// comment \n$x = 1;\n"),
            (crate::language::LapceLanguage::Python, "# comment \nx = 1\n"),
            (crate::language::LapceLanguage::Rust, "// comment \nlet x = 1;\n"),
        ] {
            let snapshot = lexilla::highlight(language, &Rope::from(text)).unwrap();
            let offset = text.find("comment ").unwrap() + "comment ".len();
            assert!(!permits_partner(&snapshot, offset), "{language:?}: end of line comment");
        }
    }

    #[test]
    fn pairing_context_stale_missing_and_wrong_length_snapshots_cannot_add() {
        let text = "let value = 1;\n";
        let snapshot = lexilla::highlight(crate::language::LapceLanguage::Rust, &Rope::from(text)).unwrap();
        assert!(permits_current_partner(Some(&snapshot), 7, 7, text.len(), 0));
        assert!(!permits_current_partner(Some(&snapshot), 6, 7, text.len(), 0));
        assert!(!permits_current_partner(Some(&snapshot), 8, 7, text.len(), 0));
        assert!(!permits_current_partner(Some(&snapshot), 7, 7, text.len()+1, 0));
        assert!(!permits_current_partner(None, 7, 7, text.len(), 0));
    }

    #[test]
    fn pairing_context_native_mixed_carets_follow_real_snapshot() {
        use crate::{buffer::Buffer, cursor::{Cursor, CursorMode}, editor::{Action, PairingOptions}, selection::{Selection, SelRegion}};
        let text = "// λ comment \nlet value = \"a value\";\n\n";
        let snapshot = lexilla::highlight(crate::language::LapceLanguage::Rust, &Rope::from(text)).unwrap();
        let comment = text.find("comment ").unwrap()+8;
        let string = text.find("a value").unwrap()+2;
        let mut selection = Selection::caret(comment);
        selection.add_region(SelRegion::caret(string));
        selection.add_region(SelRegion::caret(text.len()));
        let mut buffer = Buffer::new(&text);
        let mut cursor = Cursor::new(CursorMode::Insert(selection), None, None);
        Action::insert_with_partner_filter(&mut cursor, &mut buffer, "(", &|_,_,_|None,
            PairingOptions::legacy(true,true), &|offset| permits_partner(&snapshot,offset));
        let expected = text.replace("comment ", "comment (").replace("a value", "a (value")+"()";
        assert_eq!(buffer.to_string(), expected);
        buffer.do_undo().unwrap(); assert_eq!(buffer.to_string(), text);
        buffer.do_redo().unwrap(); assert_eq!(buffer.to_string(), expected);
    }

    #[test]
    fn pairing_context_all_openers_use_real_language_context_and_history() {
        use crate::{buffer::Buffer, cursor::{Cursor,CursorMode}, editor::{Action,PairingOptions}, selection::Selection};
        use crate::language::LapceLanguage::*;
        for (language,text) in [
            (Php,"<?php\n// note here\n$x = \"a value\";\n\n"),
            (Python,"# note here\nx = \"a value\"\n\n"),
            (Rust,"// note here\nlet x = \"a value\";\n\n"),
            (Javascript,"// note here\nlet x = \"a value\";\n\n"),
            (Typescript,"// note here\nlet x: string = \"a value\";\n\n"),
            (Css,"/* note here */\na {content: \"a value\";}\n\n"),
            (Html,"<!-- note here -->\n<a title=\"a value\">x</a>\n\n"),
            (Blade,"{{-- note here --}}\n<a title=\"a value\">x</a>\n\n"),
            (Vue,"<!-- note here -->\n<template><a title=\"a value\">x</a></template>\n\n"),
        ] {
            let snapshot=lexilla::highlight(language,&Rope::from(text)).unwrap();
            for offset in [text.find("here").unwrap()+1,text.find("a value").unwrap()+2] {
                for opener in ['(', '[', '{', '\"', '\'', '`'] {
                    let mut buffer=Buffer::new(text);
                    let mut cursor=Cursor::new(CursorMode::Insert(Selection::caret(offset)),None,None);
                    Action::insert_with_partner_filter(&mut cursor,&mut buffer,&opener.to_string(),&|_,_,_|None,
                        PairingOptions::legacy(true,true),&|offset|permits_partner(&snapshot,offset));
                    let mut expected=text.to_string();expected.insert(offset,opener);
                    assert_eq!(buffer.to_string(),expected,"{language:?} {opener} at {offset}");
                    buffer.do_undo().unwrap();assert_eq!(buffer.to_string(),text);
                    buffer.do_redo().unwrap();assert_eq!(buffer.to_string(),expected);
                }
            }
        }
    }

    #[test]
    fn pairing_context_go_has_no_lexilla_snapshot() {
        assert!(lexilla::highlight(crate::language::LapceLanguage::Go, &Rope::from("package main\n")).is_none());
    }

    #[test]
    fn pairing_context_unterminated_eof_and_unicode_are_safe() {
        for text in ["let x = \"λ value", "// λ comment"] {
            let snapshot = lexilla::highlight(crate::language::LapceLanguage::Rust, &Rope::from(text)).unwrap();
            assert!(!permits_partner(&snapshot, text.len()));
        }
    }
}

/// Existing language-unaware structural fallback. Prefer the bracket directly
/// under the caret before looking for an enclosing pair.
pub fn fallback_bracket_pair(text: &lapce_xi_rope::Rope, offset: usize) -> Option<(usize,usize)> {
    use crate::word::WordCursor;
    if offset > text.len() || text.at_or_prev_codepoint_boundary(offset) != Some(offset) { return None; }
    if let Some(other)=WordCursor::new(text,offset).match_pairs() {
        return Some((offset.min(other),offset.max(other)));
    }
    if let Some(previous) = offset.checked_sub(1).and_then(|offset| text.at_or_prev_codepoint_boundary(offset)) {
        if let Some(other)=WordCursor::new(text,previous).match_pairs() {
            return Some((previous.min(other),previous.max(other)));
        }
    }
    WordCursor::new(text,offset).find_enclosing_pair()
}

#[cfg(test)]
mod bracket_fallback_tests {
    use super::*;
    use lapce_xi_rope::Rope;
    #[test]
    fn bracket_fallback_covers_on_and_inside_every_structural_pair() {
        for pair in ["()","[]","{}"] {
            let text=Rope::from(pair);
            assert_eq!(fallback_bracket_pair(&text,0),Some((0,1)));
            assert_eq!(fallback_bracket_pair(&text,1),Some((0,1)));
        }
        let text=Rope::from("λ{[()]}\n");
        for (offset,expected) in [(2,(2,7)),(3,(3,6)),(4,(4,5)),(5,(4,5)),(6,(3,6)),(7,(2,7))] {
            assert_eq!(fallback_bracket_pair(&text,offset),Some(expected));
        }
        assert_eq!(fallback_bracket_pair(&text,1),None);
        assert_eq!(fallback_bracket_pair(&text,usize::MAX),None);
        assert_eq!(fallback_bracket_pair(&Rope::from("("),0),None);
        assert_eq!(fallback_bracket_pair(&Rope::from("()"),2),Some((0,1)));
        assert_eq!(fallback_bracket_pair(&Rope::from("{()}"),3),Some((0,3)));
        assert_eq!(fallback_bracket_pair(&Rope::from("{() x}"),3),Some((1,2)));
        assert_eq!(fallback_bracket_pair(&Rope::from("()[]"),2),Some((2,3)));
        assert_eq!(fallback_bracket_pair(&Rope::from("λ()"),4),Some((2,3)));
    }
}
