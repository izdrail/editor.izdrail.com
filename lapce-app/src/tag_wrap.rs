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
pub fn wrap(selected: &str, spec: &str, indent: &str, nl: &str) -> String {
    let name = tag_name(spec);
    if selected.contains('\n') {
        let body = selected.trim_matches(|c| c == '\r' || c == '\n');
        format!("<{spec}>{nl}{body}{nl}{indent}</{name}>")
    } else {
        format!("<{spec}>{selected}</{name}>")
    }
}

/// The element name of a typed spec such as `div class="card"`.
pub fn tag_name(spec: &str) -> &str {
    spec.split_whitespace().next().unwrap_or("")
}

/// A typed wrap spec: a valid name, optionally followed by attributes. The
/// attributes may not contain `<`, `>` or a line break.
pub fn is_valid_spec(spec: &str) -> bool {
    let spec = spec.trim();
    is_valid_tag(tag_name(spec)) && !spec.contains(['<', '>', '\n', '\r']) && spec.len() <= 200
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
    fn typed_spec_with_attributes() {
        assert_eq!(
            wrap("hi", "div class=\"card p-4\"", "", "\n"),
            "<div class=\"card p-4\">hi</div>"
        );
        assert_eq!(wrap("a\nb", "section id=x", "", "\n"), "<section id=x>\na\nb\n</section>");
        assert!(is_valid_spec("div class=\"a\""));
        assert!(is_valid_spec("x-card"));
        assert!(!is_valid_spec(""));
        assert!(!is_valid_spec("div><script>"));
        assert!(!is_valid_spec("1div"));
        assert!(!is_valid_spec(&format!("div {}", "a".repeat(300))));
        assert_eq!(tag_name("  "), "");
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
