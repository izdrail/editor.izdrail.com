//! Auto-close HTML tags in HTML, Twig, Blade, Vue and similar templates:
//! typing `>` after `<div class="x"` adds `</div>` after the caret.
//!
//! Only element tags are handled (Blade components `<x-alert>`, Livewire
//! `<livewire:...>` and Twig `<twig:...>` included). Void elements,
//! self-closing tags, PHP open tags, comparisons, tags whose attribute quotes
//! are unbalanced and tags already closed right after the caret are left
//! alone. Plain `.php` files are not touched.
use std::path::Path;

const LOOKBACK: usize = 4096;
const VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
    "source", "track", "wbr",
];

/// Whether `path` is a template where `<tag>` should auto-close.
pub fn enabled_for(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    if name.ends_with(".blade.php") {
        return true;
    }
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref(),
        Some("html" | "htm" | "xhtml" | "twig" | "vue" | "phtml" | "svelte")
    )
}

/// `before` is the text up to and including the typed `>`; `after` the text
/// following the caret. Returns the closing tag to insert after the caret.
pub fn closing_tag(before: &str, after: &str) -> Option<String> {
    let body = before.strip_suffix('>')?;
    let mut start = body.len().saturating_sub(LOOKBACK);
    while !body.is_char_boundary(start) {
        start += 1;
    }
    let window = &body[start..];
    let lt = window.rfind('<')?;
    let tag = &window[lt + 1..];
    if tag.contains('<') || tag.ends_with('/') || tag.ends_with('=') || tag.ends_with('-') {
        return None;
    }
    let name: String = tag
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '-' | '_'))
        .collect();
    if name.is_empty() || !name.chars().next()?.is_ascii_alphabetic() {
        return None;
    }
    let rest = &tag[name.len()..];
    if !rest.is_empty() && !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    if rest.matches('"').count() % 2 == 1 || rest.matches('\'').count() % 2 == 1 {
        return None;
    }
    // A `{{ ... }}` or `{% ... %}` left open means the `>` is part of it.
    if rest.matches("{{").count() > rest.matches("}}").count()
        || rest.matches("{%").count() > rest.matches("%}").count()
    {
        return None;
    }
    if VOID.contains(&name.to_lowercase().as_str()) {
        return None;
    }
    let close = format!("</{name}>");
    let mut head = after.char_indices().nth(200).map(|(i, _)| &after[..i]).unwrap_or(after);
    head = head.trim_start_matches([' ', '\t']);
    if head.starts_with(&close[..close.len() - 1]) {
        return None;
    }
    Some(close)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(before: &str) -> Option<String> {
        closing_tag(before, "")
    }

    #[test]
    fn closes_simple_and_attribute_tags() {
        assert_eq!(c("<div>").as_deref(), Some("</div>"));
        assert_eq!(c("  <a href=\"/x\" class='y'>").as_deref(), Some("</a>"));
        assert_eq!(c("<ul>\n  <li>").as_deref(), Some("</li>"));
    }

    #[test]
    fn closes_component_tags() {
        assert_eq!(c("<x-alert type=\"error\">").as_deref(), Some("</x-alert>"));
        assert_eq!(c("<livewire:search-box>").as_deref(), Some("</livewire:search-box>"));
        assert_eq!(c("<x-slot:title>").as_deref(), Some("</x-slot:title>"));
        assert_eq!(c("<x-forms.input>").as_deref(), Some("</x-forms.input>"));
    }

    #[test]
    fn multiline_attributes() {
        assert_eq!(c("<div\n  class=\"a\"\n  id=\"b\"\n>").as_deref(), Some("</div>"));
    }

    #[test]
    fn skips_void_self_closing_and_php() {
        assert_eq!(c("<br>"), None);
        assert_eq!(c("<img src=\"a\">"), None);
        assert_eq!(c("<input type=\"text\">"), None);
        assert_eq!(c("<div />"), None);
        assert_eq!(c("<x-alert/>"), None);
        assert_eq!(c("<?php echo 1; ?>"), None);
        assert_eq!(c("<?php"), None);
        assert_eq!(c("</div>"), None);
        assert_eq!(c("<!-- note -->"), None);
        assert_eq!(c("<!DOCTYPE html>"), None);
    }

    #[test]
    fn skips_arrows_comparisons_and_open_quotes() {
        assert_eq!(c("{{ $a->b>"), None);
        assert_eq!(c("{{ $a => $b>"), None);
        assert_eq!(c("if ($a<$b>"), None);
        assert_eq!(c("<a title=\"x>"), None);
        assert_eq!(c("<a title='x>"), None);
        assert_eq!(c("<a href=\"{{ route('x') }}\">").as_deref(), Some("</a>"));
        assert_eq!(c("<a {{ $x>"), None);
        assert_eq!(c("<a {% if x>"), None);
        assert_eq!(c("a > b"), None);
        assert_eq!(c(">"), None);
        assert_eq!(c(""), None);
    }

    #[test]
    fn skips_when_already_closed() {
        assert_eq!(closing_tag("<div>", "</div>"), None);
        assert_eq!(closing_tag("<div>", "  </div>\n"), None);
        assert_eq!(closing_tag("<div>", "</span>").as_deref(), Some("</div>"));
    }

    #[test]
    fn non_ascii_and_huge_inputs() {
        assert_eq!(c("<p>é</p><ü>"), None);
        assert_eq!(c("<é>"), None);
        let big = format!("{}<b>", "é".repeat(100_000));
        assert_eq!(c(&big).as_deref(), Some("</b>"));
        let nolt = format!("{}>", "x".repeat(100_000));
        assert_eq!(c(&nolt), None);
        assert_eq!(closing_tag("<b>", &"é".repeat(10_000)).as_deref(), Some("</b>"));
    }

    #[test]
    fn enabled_paths() {
        assert!(enabled_for(Path::new("a.html")));
        assert!(enabled_for(Path::new("a.html.twig")));
        assert!(enabled_for(Path::new("resources/views/a.blade.php")));
        assert!(enabled_for(Path::new("A.VUE")));
        assert!(!enabled_for(Path::new("a.php")));
        assert!(!enabled_for(Path::new("a.rs")));
    }
}
