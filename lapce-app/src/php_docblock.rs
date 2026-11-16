//! PHP docblock Tab expansion grounded in the current lexer, not stale styles.
use lapce_core::{language::LapceLanguage, lexilla};
use lapce_xi_rope::Rope;

#[derive(Debug, PartialEq, Eq)]
pub struct Expansion {
    pub insert: String,
    /// Byte distance from the original caret to the new description caret.
    pub caret: usize,
}

pub fn expansion(source: &str, offset: usize) -> Option<Expansion> {
    if source.len() > 4 * 1024 * 1024 || offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let start = source[..offset].rfind('\n').map_or(0, |i| i + 1);
    let before = &source[start..offset];
    let indent = before.trim_end_matches("/**");
    if before.strip_suffix("/**") != Some(indent) || !indent.bytes().all(|b| matches!(b, b' ' | b'\t')) {
        return None;
    }
    let end = source[offset..].find('\n').map_or(source.len(), |i| offset + i);
    if !source[offset..end].trim_matches([' ', '\t', '\r']).is_empty() {
        return None;
    }
    // If a closing delimiter already exists before the next actual code, this
    // is a partially written existing block. Never add a second closer.
    if source[end..].trim_start().starts_with("*/") || source[end..].trim_start().starts_with('*') {
        return None;
    }
    let opener = offset.checked_sub(3)?;
    let snapshot = lexilla::highlight(LapceLanguage::Php, &Rope::from(source))?;
    let style = *snapshot.styles().get(opener)?;
    if snapshot.scope(style) != Some("comment")
        || snapshot.styles().get(opener..offset)?.iter().any(|s| *s != style)
        || (opener > 0 && snapshot.styles().get(opener - 1) == Some(&style)) {
        return None;
    }
    let newline = if source[..offset].ends_with('\r') || source.contains("\r\n") { "\r\n" } else { "\n" };
    let first = format!("{newline}{indent} * ");
    Some(Expansion {
        caret: first.len(),
        insert: format!("{first}{newline}{indent} */"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closes_docblock_and_positions_description_caret() {
        let source = "<?php\n    /**\n    function example() {}\n";
        let offset = source.find("/**").unwrap() + 3;
        let e = expansion(source, offset).unwrap();
        assert_eq!(e.insert, "\n     * \n     */");
        assert_eq!(&e.insert[..e.caret], "\n     * ");
        let mut final_text = source.to_string(); final_text.insert_str(offset, &e.insert);
        assert!(final_text.contains("/**\n     * \n     */\n    function"));
    }
    #[test]
    fn preserves_tabs_crlf_and_unicode_offsets() {
        let source = "<?php\r\n// λ\r\n\t/**\r\n";
        let e = expansion(source, source.find("/**").unwrap() + 3).unwrap();
        assert_eq!(e.insert, "\r\n\t * \r\n\t */");
        assert_eq!(e.caret, "\r\n\t * ".len());
        assert!(expansion(source, source.find('λ').unwrap() + 1).is_none());
        assert!(expansion(source, source.len() + 1).is_none());
    }
    #[test]
    fn refuses_existing_blocks_strings_heredocs_and_html() {
        for source in [
            "<?php\n/** */",
            "<?php\n/**\n */",
            "<?php\n/**\n * existing\n */",
            "<?php\n/* outer\n/**",
            "<?php\n/** outer\n/**",
            "<?php\n$x = '\n/**",
            "<?php\n$x = \"\n/**",
            "<?php\n$x = <<<'TXT'\n/**\nTXT;",
            "<html>\n/**",
            "<?php ?>\n/**",
            "<?php\n/*",
            "<?php\n/***",
            "<?php\nfoo /**",
        ] {
            let offset = source.rfind("/**").map_or(source.len(), |i| i + 3);
            assert_eq!(expansion(source, offset), None, "{source}");
        }
    }
    #[test]
    fn works_in_a_mixed_template_php_region() {
        let source = "<div>Hi</div><?php\n/**";
        assert!(expansion(source, source.len()).is_some());
    }
}
