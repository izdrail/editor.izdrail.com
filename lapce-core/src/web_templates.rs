//! Byte-preserving composition of template and vendored web-language snapshots.
use lexilla_rs::{Property, Snapshot};

pub fn twig_lists() -> Vec<String> {
    let mut lists = vec!["and or not in is matches starts ends with true false null none elseif else endif endfor endblock endmacro endset endapply endwith endembed endautoescape endverbatim verbatim autoescape".to_owned(), String::new(), String::new(), String::new()];
    for s in crate::web_metadata::symbols()
        .iter()
        .filter(|s| s.framework == "twig")
    {
        let index = match s.kind.as_str() {
            "tag" => 0,
            "filter" => 1,
            "function" => 2,
            "test" => 3,
            _ => continue,
        };
        lists[index].push(' ');
        lists[index].push_str(&s.name);
    }
    lists
}

pub fn twig(source: &str) -> Option<Snapshot> {
    let lists = twig_lists();
    let words: Vec<&str> = lists.iter().map(String::as_str).collect();
    let template = Snapshot::lex("twig", source, &words).ok()?;
    // Preserve newlines and byte offsets while hiding Twig regions from HTML.
    // UTF-8 bytes in hidden regions become ASCII spaces of identical length.
    let mut masked = source.as_bytes().to_vec();
    for (i, &style) in template.styles().iter().enumerate() {
        if style != 0 && masked[i] != b'\n' && masked[i] != b'\r' {
            masked[i] = b' ';
        }
    }
    let masked = String::from_utf8(masked).ok()?;
    let mut host = Snapshot::lex(
        "hypertext",
        &masked,
        &[
            "a body div head html input link meta p script span style title",
            "async await const function let new return var",
        ],
    )
    .ok()?;
    embed_web(&masked, &mut host);
    host.overlay(0, &template, true);
    Some(host)
}

/// JSX/TSX: the jsx lexer paints markup regions (tags, attributes,
/// delimiters, expression braces); mask those bytes and lex the remainder
/// with the cpp lexer's JavaScript mode, then overlay the markup. Children
/// text and `{expression}` interiors keep style 0 in the markup snapshot, so
/// the code pass styles them; nested markup inside expressions is painted by
/// the jsx lexer and masked back out of the code pass.
pub fn jsx(source: &str, typescript: bool) -> Option<Snapshot> {
    let markup = Snapshot::lex("jsx", source, &[]).ok()?;
    let mut masked = source.as_bytes().to_vec();
    for (i, &style) in markup.styles().iter().enumerate() {
        if style != 0 && masked[i] != b'\n' && masked[i] != b'\r' {
            masked[i] = b' ';
        }
    }
    let masked = String::from_utf8(masked).ok()?;
    let js_words = "async await break case catch class const continue debugger default delete do else enum export extends false finally for from function if implements import in instanceof interface let new null of package private protected public return static super switch this throw true try typeof var void while with yield";
    let ts_words = "abstract accessor any as asserts async await bigint boolean break case catch class const constructor continue debugger declare default delete do else enum export extends false finally for from function get if implements import in infer instanceof interface intrinsic is keyof let module namespace never new null number object of override package private protected public readonly require return satisfies set static string super switch symbol this throw true try type typeof undefined unique unknown var void while with yield";
    let words: &[&str] = if typescript { &[ts_words] } else { &[js_words] };
    let mut host =
        Snapshot::lex_with_properties("cpp", &masked, words, &[Property::Javascript(true)])
            .ok()?;
    host.overlay(0, &markup, true);
    Some(host)
}

/// Lexilla's HTML lexer handles script regions but not modern CSS. Delegate
/// recognised inline JS/CSS bodies. Do not overwrite PHP/Blade/Twig constructs
/// or non-JS scripts (JSON/import maps). Unknown MIME modes stay with the host.
pub fn embed_web(source: &str, host: &mut Snapshot) {
    let lower = source.to_ascii_lowercase();
    for (tag, lexer) in [("script", "cpp"), ("style", "css")] {
        let mut cursor = 0;
        while let Some(found) = lower[cursor..].find(&format!("<{tag}")) {
            let start = cursor + found;
            let after = start + tag.len() + 1;
            if lower
                .as_bytes()
                .get(after)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'-')
            {
                cursor = after;
                continue;
            }
            // Reject tag-like text inside comments/strings/code regions.
            if host.scope(host.styles()[start]) != Some("tag") {
                cursor = after;
                continue;
            }
            let Some(end_tag) = lower[after..].find('>') else {
                break;
            };
            let body = after + end_tag + 1;
            let Some(close) = lower[body..].find(&format!("</{tag}")) else {
                break;
            };
            let end = body + close;
            let attrs = &lower[after..body];
            if attrs.contains("src=")
                || (attrs.contains("type=")
                    && !attrs.contains("javascript")
                    && !attrs.contains("module")
                    && !attrs.contains("text/css"))
            {
                cursor = end;
                continue;
            }
            let text = &source[body..end];
            // Template syntax is left with the host until injection spans are
            // available. This avoids swallowing directives as JS/CSS strings.
            if text.contains("{{")
                || text.contains("{!!")
                || text.contains("<?")
                || (tag == "script" && text.contains('@'))
            {
                cursor = end;
                continue;
            }
            let properties = if lexer == "cpp" {
                vec![Property::Javascript(true)]
            } else {
                Vec::new()
            };
            let words: &[&str] = if lexer == "cpp" {
                &[
                    "async await break case catch class const continue debugger default delete do else export extends false finally for from function if import in instanceof let new null of return static super switch this throw true try typeof var void while with yield",
                ]
            } else {
                &[]
            };
            if let Ok(embedded) =
                Snapshot::lex_with_properties(lexer, text, words, &properties)
            {
                host.overlay(body, &embedded, false);
            }
            cursor = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope(s: &Snapshot, t: &str, word: &str) -> Option<&'static str> {
        s.scope(s.styles()[t.find(word).unwrap()])
    }
    #[test]
    fn twig_regions_quotes_verbatim_and_embeddings() {
        let t = "<div>{{- user.name|upper -}}</div>{% if x is defined %}{# {{ hidden }} #}{% endif %}\n{{ {'x': '}}'} }}\n{% verbatim %}{{ untouched }}{% endverbatim %}\n<script>const x = `hi`;</script><style>body { color: red; }</style>";
        let s = twig(t).unwrap();
        assert_eq!(s.len(), t.len());
        assert_eq!(scope(&s, t, "<div"), Some("tag"));
        assert_eq!(scope(&s, t, "upper"), Some("function"));
        assert_eq!(scope(&s, t, "hidden"), Some("comment"));
        assert_eq!(scope(&s, t, "untouched"), Some("string"));
        assert_eq!(scope(&s, t, "const"), Some("keyword"));
        assert_eq!(scope(&s, t, "`hi`"), Some("string"));
        assert_ne!(scope(&s, t, "color"), Some("string"));
    }
    #[test]
    fn incomplete_inputs_utf8_crlf_edits_and_undo() {
        for t in [
            "",
            "λ🙂\r\n{{",
            "{{ '}}",
            "{# λ",
            "{% verbatim %}{{ x }}",
            "<script>const x = {{ value }};</script>",
            "{% if (x %}",
            "{{ {'a': 1} }}",
        ] {
            assert_eq!(twig(t).unwrap().len(), t.len());
            let edited = format!("{t} }} }}");
            assert_eq!(twig(&edited).unwrap().len(), edited.len());
            assert_eq!(twig(t).unwrap().styles(), twig(t).unwrap().styles());
        }
    }
    #[test]
    fn routing() {
        use crate::language::LapceLanguage;
        assert_eq!(
            LapceLanguage::from_path(std::path::Path::new("home.html.twig")),
            LapceLanguage::Twig
        );
        for (file, lang) in [
            ("x.css", LapceLanguage::Css),
            ("x.scss", LapceLanguage::Scss),
            ("x.js", LapceLanguage::Javascript),
            ("x.ts", LapceLanguage::Typescript),
            ("x.jsx", LapceLanguage::Jsx),
            ("x.tsx", LapceLanguage::Tsx),
        ] {
            assert_eq!(LapceLanguage::from_path(std::path::Path::new(file)), lang);
            assert!(
                crate::lexilla::highlight(
                    lang,
                    &lapce_xi_rope::Rope::from("const x = 1;")
                )
                .is_some()
            );
        }
    }
}

#[cfg(test)]
mod jsx_tests {
    use super::*;
    fn scope_pos(s: &Snapshot, t: &str, word: &str) -> Option<&'static str> {
        s.scope(s.styles()[t.find(word).unwrap()])
    }
    fn has_tag(s: &Snapshot) -> bool {
        (0..s.len()).any(|i| s.scope(s.styles()[i]) == Some("tag"))
    }
    #[test]
    fn markup_regions_compose_with_javascript() {
        let t = "return <div className=\"a b\" onClick={go}>Hi {name}</div>;";
        let s = jsx(t, false).unwrap();
        assert_eq!(s.len(), t.len());
        assert_eq!(scope_pos(&s, t, "return"), Some("keyword"));
        assert_eq!(scope_pos(&s, t, "div"), Some("tag"));
        // Attribute names stay default-coloured, consistent with HTML.
        assert_eq!(scope_pos(&s, t, "className"), None);
        assert_eq!(scope_pos(&s, t, "\"a b\""), Some("string"));
        assert_eq!(scope_pos(&s, t, "div>;"), Some("tag")); // closing tag name
        // Expression interiors are styled by the code pass: identifiers uncoloured.
        assert_eq!(scope_pos(&s, t, "name}"), None);
    }
    #[test]
    fn comparisons_generics_and_types_stay_code() {
        for t in [
            "let r = a < b;",
            "foo<T>(x);",
            "const f = <T,>(x: T) => x;",
            "const g = <T extends U>(x: T) => x;",
            "const n = useState<App>();",
            "let s: Array<string> = [];",
        ] {
            let s = jsx(t, false).unwrap();
            assert_eq!(s.len(), t.len());
            assert!(!has_tag(&s), "{t}");
        }
    }
    #[test]
    fn markup_inside_strings_comments_and_regex_stays_code() {
        let s = jsx("const s = \"<div>\";", false).unwrap();
        assert_eq!(scope_pos(&s, "const s = \"<div>\";", "div"), Some("string"));
        let s = jsx("const s = `<div>`;", false).unwrap();
        assert_eq!(scope_pos(&s, "const s = `<div>`;", "div"), Some("string"));
        let s = jsx("// <div>", false).unwrap();
        assert_eq!(scope_pos(&s, "// <div>", "div"), Some("comment"));
        assert!(!has_tag(&jsx("x.replace(/</g, \"</\");", false).unwrap()));
    }
    #[test]
    fn nested_elements_expressions_and_template_substitutions() {
        let t = "return <ul>{items.map(i => <li key={i}>{i}</li>)}</ul>;";
        let s = jsx(t, false).unwrap();
        assert_eq!(s.len(), t.len());
        assert_eq!(scope_pos(&s, t, "ul"), Some("tag"));
        assert_eq!(scope_pos(&s, t, "li"), Some("tag"));
        assert_eq!(scope_pos(&s, t, "key"), None);
        let t = "return <div>{`t ${<span>x</span>}`}</div>;";
        let s = jsx(t, false).unwrap();
        assert_eq!(scope_pos(&s, t, "span"), Some("tag"));
        // Fragments paint nothing (no names); children and code stay code.
        let s = jsx("return <>hi</>;", false).unwrap();
        assert!(!has_tag(&s));
        assert_eq!(scope_pos(&s, "return <>hi</>;", "return"), Some("keyword"));
    }
    #[test]
    fn tsx_adds_type_keywords() {
        let t = "type Row = { id: number };";
        assert_ne!(scope_pos(&jsx(t, false).unwrap(), t, "type"), Some("keyword"));
        let s = jsx(t, true).unwrap();
        assert_eq!(scope_pos(&s, t, "type"), Some("keyword"));
        assert_eq!(scope_pos(&s, t, "number"), Some("keyword"));
    }
    #[test]
    fn incomplete_utf8_and_crlf_inputs() {
        for t in [
            "",
            "λ🙂\r\nreturn <div",
            "return <div className=\"x\"",
            "return <>{x}",
            "<T extends",
            "x = a <",
        ] {
            assert_eq!(jsx(t, false).unwrap().len(), t.len(), "{t}");
        }
    }
}

/// The unfinished final Twig region determines the popup's symbol category.
pub fn completion_kind(before: &str) -> Option<&'static str> {
    let lists = twig_lists();
    let refs: Vec<&str> = lists.iter().map(String::as_str).collect();
    let template = Snapshot::lex("twig", before, &refs).ok()?;
    let last = *template.styles().last()?;
    if matches!(last, 0 | 2 | 3 | 9) {
        return None;
    }
    let echo = before.rfind("{{");
    let tag = before.rfind("{%");
    let start = match (echo, tag) {
        (Some(a), Some(b)) => a.max(b),
        (Some(a), None) | (None, Some(a)) => a,
        _ => return None,
    };
    let tail = &before[start + 2..];
    // Ignore the identifier currently being completed.
    let context = tail
        .trim_end_matches(|c: char| c.is_alphanumeric() || c == '_')
        .trim_end();
    if context.ends_with('|') {
        Some("filter")
    } else if context.split_whitespace().last() == Some("is") {
        Some("test")
    } else if tag == Some(start)
        && !tail
            .trim_start_matches(['-', '~'])
            .trim_start()
            .contains(char::is_whitespace)
    {
        Some("tag")
    } else {
        Some("function")
    }
}

pub fn php_completion_context(before: &str) -> bool {
    let pure = crate::lexilla::is_pure_php(before);
    let Ok(snapshot) = Snapshot::lex(
        if pure { "php" } else { "hypertext" },
        before,
        if pure {
            crate::lexilla::PHP_WORD_LISTS
        } else {
            &[]
        },
    ) else {
        return false;
    };
    if let Some(&style) = snapshot.styles().last() {
        if matches!(snapshot.scope(style), Some("string" | "comment")) {
            return false;
        }
        if (pure && style == 21) || (!pure && !(118..=127).contains(&style)) {
            return false;
        }
    }
    let context = before
        .trim_end_matches(|c: char| c.is_alphanumeric() || c == '_')
        .trim_end();
    !context.ends_with("->") && !context.ends_with("::")
}

#[cfg(test)]
mod isolation_tests {
    use super::*;
    #[test]
    fn popup_positions_and_host_strings_are_separate() {
        assert_eq!(completion_kind("{{ x|up"), Some("filter"));
        assert_eq!(completion_kind("{{ x is def"), Some("test"));
        assert_eq!(completion_kind("{% fo"), Some("tag"));
        assert_eq!(completion_kind("{# x|up"), None);
        assert_eq!(completion_kind("{{ 'x|up"), None);
        assert_eq!(completion_kind("{% verbatim %}{{ x|up"), None);
        assert!(!php_completion_context("<?php // strlen"));
        assert!(!php_completion_context("<?php 'strlen"));
        assert!(!php_completion_context("<?php $x->str"));
        assert!(php_completion_context("<?php str"));
        assert!(!php_completion_context("<div>str"));
    }
    #[test]
    fn composed_snapshot_does_not_relabel_existing_scopes() {
        let t = "<div>{{ 's' }}</div><p>{# c #}</p><script>const x = `s`;</script>";
        let s = twig(t).unwrap();
        for word in ["<div", "</div", "<p>", "</p>"] {
            assert_eq!(s.scope(s.styles()[t.find(word).unwrap()]), Some("tag"));
        }
        assert_eq!(s.scope(s.styles()[t.find("{#").unwrap()]), Some("comment"));
    }
}
