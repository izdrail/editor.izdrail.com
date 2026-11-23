//! Pair insertion context from the existing lexer snapshot, never a second parser.
use lexilla_rs::Snapshot;

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
    if comment(current) || (offset == snapshot.len() && comment(previous)) { return false; }
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
