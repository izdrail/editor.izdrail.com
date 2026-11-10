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
    let words: &[&str] = if typescript { &[crate::lexilla::TYPESCRIPT_KEYWORDS] } else { &[js_words] };
    let mut host =
        Snapshot::lex_with_properties("cpp", &masked, words, &[Property::Javascript(true)])
            .ok()?;
    host.overlay(0, &markup, true);
    Some(host)
}

/// Read an opening tag without letting a quoted `>` end it early. Attribute
/// names are exact tokens, not substring matches inside data-* or values.
fn web_tag_attributes(source: &str, after: usize) -> Option<(usize, Vec<(&str, &str)>)> {
    let bytes = source.as_bytes();
    let mut i = after;
    let mut attributes = Vec::new();
    while i < bytes.len() {
        while bytes.get(i).is_some_and(u8::is_ascii_whitespace) { i += 1; }
        match bytes.get(i)? {
            b'>' => return Some((i + 1, attributes)),
            b'/' => { i += 1; continue; }
            _ => {}
        }
        let start = i;
        while bytes.get(i).is_some_and(|b| !b.is_ascii_whitespace() && !matches!(b, b'=' | b'>' | b'/')) { i += 1; }
        if i == start { return None; }
        let name = &source[start..i];
        while bytes.get(i).is_some_and(u8::is_ascii_whitespace) { i += 1; }
        let mut value = "";
        if bytes.get(i) == Some(&b'=') {
            i += 1;
            while bytes.get(i).is_some_and(u8::is_ascii_whitespace) { i += 1; }
            let quote = bytes.get(i).copied()?;
            if matches!(quote, b'\'' | b'"') {
                i += 1;
                let start = i;
                while bytes.get(i).is_some_and(|b| *b != quote) { i += 1; }
                bytes.get(i)?;
                value = &source[start..i];
                i += 1;
            } else {
                let start = i;
                while bytes.get(i).is_some_and(|b| !b.is_ascii_whitespace() && *b != b'>') { i += 1; }
                value = &source[start..i];
            }
        }
        // HTML keeps the first duplicate attribute.
        if !attributes.iter().any(|(key, _)| *key == name) { attributes.push((name, value)); }
    }
    None
}

fn web_script_type(value: &str) -> bool {
    matches!(value.trim_matches(|c: char| c.is_ascii_whitespace()),
        "" | "module" | "text/javascript" | "application/javascript"
        | "application/ecmascript" | "text/ecmascript" | "application/x-javascript"
        | "application/x-ecmascript" | "text/jscript" | "text/livescript"
        | "text/x-javascript" | "text/x-ecmascript"
        | "text/javascript1.0" | "text/javascript1.1" | "text/javascript1.2"
        | "text/javascript1.3" | "text/javascript1.4" | "text/javascript1.5")
}

/// HTML raw-text end tags need an exact tag-name boundary. A string such as
/// </scripture> is body text, while a real </script> still ends the element
/// even when it appears inside a JavaScript string.
fn web_end_tag(source: &str, body: usize, tag: &str) -> Option<usize> {
    let needle = format!("</{tag}");
    let mut cursor = body;
    while let Some(found) = source[cursor..].find(&needle) {
        let start = cursor + found;
        let after = start + needle.len();
        if source.as_bytes().get(after).is_some_and(|b| {
            b.is_ascii_whitespace() || matches!(b, b'>' | b'/')
        }) {
            return Some(start);
        }
        cursor = after;
    }
    None
}

/// Lexilla's HTML lexer handles script regions but not modern CSS. Delegate
/// recognised inline JS/CSS bodies. Do not overwrite PHP/Blade/Twig constructs
/// or non-JS scripts (JSON/import maps). Unknown MIME modes stay with the host.
pub fn embed_web(source: &str, host: &mut Snapshot) {
    let lower = source.to_ascii_lowercase();
    for (tag, lexer, kind) in [
        ("script", "cpp", lexilla_rs::Embed::Script),
        ("style", "css", lexilla_rs::Embed::Style),
    ] {
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
            let Some((body, attrs)) = web_tag_attributes(&lower, after) else {
                break;
            };
            // While typing, a completed opener owns the remaining raw-text
            // body even before its closing tag exists.
            let end = web_end_tag(&lower, body, tag).unwrap_or(source.len());
            let attribute = |name| attrs.iter().find(|(key, _)| *key == name).map(|(_, value)| *value);
            let supported = attribute("type").is_none_or(|value| {
                if tag == "script" { web_script_type(value) }
                else { matches!(value.trim_matches(|c: char| c.is_ascii_whitespace()), "" | "text/css") }
            });
            if (tag == "script" && attribute("src").is_some()) || !supported {
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
                host.record_embed(body, end, kind);
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
    fn embedded_bodies_require_exact_closing_tag_names() {
        use crate::language::LapceLanguage;
        use lapce_xi_rope::Rope;
        for end in ["</script>", "</SCRIPT >", "</script\t>", "</script/>"] {
            let text = format!("<script>const marker = '</scripture>'; const after = 42;{end}<p>end</p>");
            let snap = crate::lexilla::highlight(LapceLanguage::Html, &Rope::from(text.as_str())).unwrap();
            assert_eq!(snap.embed_at(text.find("after").unwrap()), Some(lexilla_rs::Embed::Script), "{end}");
            assert_eq!(scope(&snap, &text, "42"), Some("constant"));
            assert_eq!(snap.embed_at(text.find("<p>").unwrap()), None);
        }
        let text = "<style>body::after { content: '</styles>'; } .after { color: red; }</style>";
        let snap = crate::lexilla::highlight(LapceLanguage::Html, &Rope::from(text)).unwrap();
        assert_eq!(snap.embed_at(text.find("color").unwrap()), Some(lexilla_rs::Embed::Style));
        assert_eq!(web_end_tag("</scripture></script-x></script_></script>", 0, "script"), Some(33));
        assert_eq!(web_end_tag("</scripture>", 0, "script"), None);
        // A genuine close tag is HTML syntax even when JS sees a quote.
        assert_eq!(web_end_tag("let s = '</script>';", 0, "script"), Some(9));
    }

    #[test]
    fn unfinished_embedded_bodies_keep_styles_and_regions_until_eof() {
        use crate::language::LapceLanguage;
        use lapce_xi_rope::Rope;
        for language in [LapceLanguage::Html, LapceLanguage::Blade, LapceLanguage::Php, LapceLanguage::Twig] {
            for (open, body, close, token, kind) in [
                ("<script>", "const value = 'λ🙂';\r\nconst after = 42;", "</script>", "after", lexilla_rs::Embed::Script),
                ("<style>", ".card { color: green; gap: 12px; }", "</style>", "color", lexilla_rs::Embed::Style),
            ] {
                let unfinished = format!("<div>host</div>\r\n{open}{body}");
                let snap = crate::lexilla::highlight(language, &Rope::from(unfinished.as_str())).unwrap();
                let offset = unfinished.find(token).unwrap();
                assert_eq!(snap.len(), unfinished.len());
                assert_eq!(snap.embed_at(offset), Some(kind), "{language:?}: {open}");
                assert_eq!(snap.embed_at(unfinished.len() - 1), Some(kind));
                let closed = format!("{unfinished}{close}<p>end</p>");
                let closed_snap = crate::lexilla::highlight(language, &Rope::from(closed.as_str())).unwrap();
                assert_eq!(closed_snap.embed_at(offset), Some(kind));
                assert_eq!(closed_snap.embed_at(closed.find("<p>").unwrap()), None);
                assert_eq!(&snap.styles()[..], &closed_snap.styles()[..unfinished.len()]);
                let undo = crate::lexilla::highlight(language, &Rope::from(unfinished.as_str())).unwrap();
                assert_eq!(undo.styles(), snap.styles());
            }
        }
        for text in ["<script type='importmap'>const value = 42;", "<script src='app.js'>const value = 42;", "<style type='text/less'>.card { color: green; }", "<style>.card { color: {{ value }}; }"] {
            let snap = crate::lexilla::highlight(LapceLanguage::Html, &Rope::from(text)).unwrap();
            assert_eq!(snap.embed_at(text.len() - 1), None, "{text}");
        }
    }

    #[test]
    fn embedded_web_types_use_real_attributes_not_substrings() {
        use crate::language::LapceLanguage;
        use lapce_xi_rope::Rope;
        for attrs in ["type = 'application/ld+json'", "TYPE=importmap", "type = speculationrules", "type='application/not-javascript'", "type='text/javascript; charset=utf-8'", "src = 'external.js'"] {
            let text = format!("<script {attrs}>const value = 42;</script>");
            let snap = crate::lexilla::highlight(LapceLanguage::Html, &Rope::from(text.as_str())).unwrap();
            assert_eq!(snap.embed_at(text.find("const").unwrap()), None, "{attrs}");
        }
        for attrs in ["type = 'module'", "TYPE = \"text/javascript\"", "data-type='importmap'", "data-src='external.js'", "title='a > b' type=module", "type=''", "type='module' type='importmap'"] {
            let text = format!("<script {attrs}>const value = 42;</script>");
            let snap = crate::lexilla::highlight(LapceLanguage::Html, &Rope::from(text.as_str())).unwrap();
            assert_eq!(snap.embed_at(text.find("const").unwrap()), Some(lexilla_rs::Embed::Script), "{attrs}");
            assert_eq!(scope(&snap, &text, "const"), Some("keyword"));
        }
        for (attrs, expected) in [("type = 'text/css'", Some(lexilla_rs::Embed::Style)), ("type='text/less'", None), ("type='text/css-module'", None)] {
            let text = format!("<style {attrs}>body {{ color: red; }}</style>");
            let snap = crate::lexilla::highlight(LapceLanguage::Html, &Rope::from(text.as_str())).unwrap();
            assert_eq!(snap.embed_at(text.find("color").unwrap()), expected, "{attrs}");
        }
        assert!(web_tag_attributes("<script title='unfinished", 7).is_none());
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
            ("x.vue", LapceLanguage::Vue),
            ("x.md", LapceLanguage::Markdown),
            ("x.markdown", LapceLanguage::Markdown),
            ("x.ini", LapceLanguage::Ini),
            (".env", LapceLanguage::Ini),
            (".env.example", LapceLanguage::Ini),
            ("x.diff", LapceLanguage::Diff),
            ("Makefile", LapceLanguage::Make),
            ("x.mk", LapceLanguage::Make),
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
        // Attribute names take the theme's attribute scope, consistent with HTML.
        assert_eq!(scope_pos(&s, t, "className"), Some("attribute"));
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
        assert_eq!(scope_pos(&s, t, "key"), Some("attribute"));
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
