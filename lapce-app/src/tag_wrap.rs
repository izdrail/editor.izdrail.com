//! "Wrap Selection in Tag": surrounds the selection (or the word at the caret)
//! with an HTML/Blade/Vue element picked from a palette.
//!
//! One line is wrapped inline (`<b>text</b>`). A multi-line selection puts
//! the tags on their own lines at the indent of its first line.

/// Tags offered by the palette: HTML containers and text, lists, tables,
/// forms, and the Blade/Livewire/Twig wrappers web developers type most.
pub const TAGS: [&str; 34] = [
    "div", "span", "p", "a", "section", "article", "header", "footer", "main", "nav",
    "aside", "ul", "ol", "li", "table", "tr", "td", "th", "form", "label", "button",
    "strong", "em", "code", "pre", "blockquote", "h1", "h2", "h3", "template",
    "x-slot", "x-card", "x-alert", "livewire:component",
];

/// The text replacing `selected` when wrapped in `tag`. `indent` is the
/// leading whitespace of the selection's first line.
pub fn wrap(selected: &str, tag: &str, indent: &str, nl: &str) -> String {
    if selected.contains('\n') {
        let body = selected.trim_matches(|c| c == '\r' || c == '\n');
        format!("<{tag}>{nl}{body}{nl}{indent}</{tag}>")
    } else {
        format!("<{tag}>{selected}</{tag}>")
    }
}

/// Closing-tag name: attributes and Livewire-style `name` suffixes kept as typed.
pub fn is_valid_tag(tag: &str) -> bool {
    let mut chars = tag.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && tag.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | ':' | '.' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_is_inline() {
        assert_eq!(wrap("hello", "b", "", "\n"), "<b>hello</b>");
        assert_eq!(wrap("", "div", "  ", "\n"), "<div></div>");
    }

    #[test]
    fn multi_line_gets_own_lines() {
        assert_eq!(
            wrap("  <li>a</li>\n  <li>b</li>", "ul", "  ", "\n"),
            "<ul>\n  <li>a</li>\n  <li>b</li>\n  </ul>"
        );
        assert_eq!(wrap("a\r\nb\r\n", "div", "", "\r\n"), "<div>\r\na\r\nb\r\n</div>");
    }

    #[test]
    fn all_offered_tags_are_valid() {
        for t in TAGS {
            assert!(is_valid_tag(t), "{t}");
        }
        assert!(!is_valid_tag(""));
        assert!(!is_valid_tag("1a"));
        assert!(!is_valid_tag("a b"));
        assert!(!is_valid_tag("a>"));
    }

    #[test]
    fn tags_are_unique() {
        let mut v = TAGS.to_vec();
        v.sort();
        v.dedup();
        assert_eq!(v.len(), TAGS.len());
    }
}
