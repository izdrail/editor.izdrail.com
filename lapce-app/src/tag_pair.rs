//! Finds the opening and closing tag names of the element around a byte
//! offset, for "Rename Tag". Textual, bounded, forgiving: comments, `<?php`,
//! `{{-- --}}`, doctype, self-closing and void tags are skipped, `<script>`
//! and `<style>` bodies are not parsed, mismatched closers are tolerated.
use std::ops::Range;

const MAX_BYTES: usize = 1024 * 1024;
const MAX_TAG: usize = 4096;
const MAX_DEPTH: usize = 512;
const VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
    "source", "track", "wbr",
];

#[derive(Debug, PartialEq, Eq)]
pub struct Pair {
    pub open_name: Range<usize>,
    pub close_name: Range<usize>,
}

struct Open {
    name: String,
    name_range: Range<usize>,
    start: usize,
    end: usize,
}

fn name_end(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && (b[i].is_ascii_alphanumeric() || matches!(b[i], b'-' | b':' | b'.' | b'_')) {
        i += 1;
    }
    i
}

/// Index just past the `>` closing the tag that starts at `from`, honouring
/// quotes and `{{ }}`. Returns (end, self_closing).
fn tag_end(b: &[u8], from: usize) -> Option<(usize, bool)> {
    let limit = b.len().min(from + MAX_TAG);
    let (mut i, mut quote, mut braces) = (from, None::<u8>, 0i32);
    while i < limit {
        let c = b[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if c == b'{' && b.get(i + 1) == Some(&b'{') {
            braces += 1;
            i += 1;
        } else if c == b'}' && b.get(i + 1) == Some(&b'}') && braces > 0 {
            braces -= 1;
            i += 1;
        } else if braces == 0 {
            match c {
                b'"' | b'\'' => quote = Some(c),
                b'<' => return None,
                b'>' => return Some((i + 1, i > from && b[i - 1] == b'/')),
                _ => {}
            }
        }
        i += 1;
    }
    None
}

fn find_from(b: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    b.get(from..)?.windows(pat.len()).position(|w| w == pat).map(|p| p + from)
}

/// The innermost element whose opening or closing tag contains `offset`.
pub fn pair_at(text: &str, offset: usize) -> Option<Pair> {
    if text.len() > MAX_BYTES {
        return None;
    }
    let b = text.as_bytes();
    let mut stack: Vec<Open> = vec![];
    let mut best: Option<(usize, Pair)> = None; // (span length, pair)
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            // Blade comments hide tags.
            if b[i] == b'{' && b[i..].starts_with(b"{{--") {
                i = find_from(b, i + 4, b"--}}").map(|p| p + 4).unwrap_or(b.len());
                continue;
            }
            i += 1;
            continue;
        }
        let rest = &b[i..];
        if rest.starts_with(b"<!--") {
            i = find_from(b, i + 4, b"-->").map(|p| p + 3).unwrap_or(b.len());
        } else if rest.starts_with(b"<?") {
            i = find_from(b, i + 2, b"?>").map(|p| p + 2).unwrap_or(b.len());
        } else if rest.starts_with(b"<!") {
            i = find_from(b, i + 2, b">").map(|p| p + 1).unwrap_or(b.len());
        } else if rest.starts_with(b"</") {
            let ns = i + 2;
            let ne = name_end(b, ns);
            let name = &text[ns..ne];
            let end = if ne > ns {
                let mut j = ne;
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                (b.get(j) == Some(&b'>')).then_some(j + 1)
            } else {
                None
            };
            let Some(end) = end else {
                i += 1;
                continue;
            };
            if let Some(pos) = stack.iter().rposition(|o| o.name == name) {
                let open = stack.swap_remove(pos);
                stack.truncate(pos);
                let hit = (open.start <= offset && offset < open.end)
                    || (i <= offset && offset <= end);
                if hit {
                    let span = end - open.start;
                    if best.as_ref().is_none_or(|(s, _)| span < *s) {
                        best = Some((
                            span,
                            Pair { open_name: open.name_range, close_name: ns..ne },
                        ));
                    }
                }
            }
            i = end;
        } else if rest.len() > 1 && rest[1].is_ascii_alphabetic() {
            let ns = i + 1;
            let ne = name_end(b, ns);
            let name = text[ns..ne].to_string();
            let Some((end, self_closing)) = tag_end(b, ne) else {
                i += 1;
                continue;
            };
            let lower = name.to_ascii_lowercase();
            if self_closing || VOID.contains(&lower.as_str()) {
                i = end;
                continue;
            }
            if lower == "script" || lower == "style" {
                let close = format!("</{lower}");
                i = text[end..]
                    .to_ascii_lowercase()
                    .find(&close)
                    .map(|p| p + end)
                    .unwrap_or(b.len());
                // Re-enter at the closing tag so it pairs with this opener.
                if stack.len() < MAX_DEPTH {
                    stack.push(Open { name, name_range: ns..ne, start: i.min(ns), end });
                }
                // start/end above are for the opening tag; fix start:
                if let Some(o) = stack.last_mut() {
                    o.start = ns - 1;
                }
                continue;
            }
            if stack.len() < MAX_DEPTH {
                stack.push(Open { name, name_range: ns..ne, start: i, end });
            }
            i = end;
        } else {
            i += 1;
        }
    }
    best.map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(text: &str, at: usize) -> Option<(String, String)> {
        pair_at(text, at).map(|p| (text[p.open_name].to_string(), text[p.close_name].to_string()))
    }

    #[test]
    fn simple_pair_from_either_tag() {
        let t = "<div><p>hi</p></div>";
        assert_eq!(names(t, 2), Some(("div".into(), "div".into())));
        assert_eq!(names(t, 16), Some(("div".into(), "div".into())));
        assert_eq!(names(t, 6), Some(("p".into(), "p".into())));
        assert_eq!(names(t, 13), Some(("p".into(), "p".into())));
    }

    #[test]
    fn caret_in_text_is_not_a_tag() {
        assert_eq!(names("<div>hi</div>", 6), None);
    }

    #[test]
    fn attributes_quotes_and_blade_expressions() {
        let t = "<a href=\"x>y\" class='{{ $a > 1 }}'>t</a>";
        assert_eq!(names(t, 20), Some(("a".into(), "a".into())));
        let t = "<x-alert :msg=\"$a > 1\">t</x-alert>";
        assert_eq!(names(t, 3), Some(("x-alert".into(), "x-alert".into())));
    }

    #[test]
    fn void_self_closing_and_nesting() {
        let t = "<ul><li><br><img src=a><x-i /></li><li>b</li></ul>";
        assert_eq!(names(t, 5), Some(("li".into(), "li".into())));
        let p = pair_at(t, 36).unwrap();
        assert_eq!(&t[p.open_name.clone()], "li");
        assert!(p.open_name.start > 30);
    }

    #[test]
    fn comments_php_and_script_are_skipped() {
        let t = "<div><!-- </div> --><?php echo '</div>'; ?>{{-- </div> --}}<script>if (a<b) { x = '</div>'; }</script></div>";
        assert_eq!(names(t, 2), Some(("div".into(), "div".into())));
        let at = t.find("<script>").unwrap() + 2;
        assert_eq!(names(t, at), Some(("script".into(), "script".into())));
    }

    #[test]
    fn mismatched_and_unclosed_do_not_panic() {
        assert_eq!(names("<div><span></div>", 2), Some(("div".into(), "div".into())));
        assert_eq!(names("<div>", 2), None);
        assert_eq!(names("</div>", 2), None);
        assert_eq!(names("<", 0), None);
        assert_eq!(names("<>", 1), None);
        assert_eq!(names("<a", 1), None);
        assert_eq!(names("", 0), None);
        assert_eq!(names("é<b>é</b>é", 2), Some(("b".into(), "b".into())));
    }

    #[test]
    fn huge_and_deep_inputs_are_bounded() {
        let deep = "<div>".repeat(5000) + &"</div>".repeat(5000);
        let _ = pair_at(&deep, 2);
        assert_eq!(pair_at(&"a".repeat(2 * 1024 * 1024), 5), None);
        let lt = "<".repeat(100_000);
        let _ = pair_at(&lt, 50_000);
        let q = format!("<a x=\"{}", "q".repeat(100_000));
        let _ = pair_at(&q, 3);
    }
}
