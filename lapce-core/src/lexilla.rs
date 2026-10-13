//! Lexilla configuration and viewport theme conversion for the editor.
use crate::language::LapceLanguage;
use lapce_rpc::style::{LineStyle, Style};
use lapce_xi_rope::Rope;
use lexilla_rs::Snapshot;
use std::sync::Arc;

pub fn highlight(language: LapceLanguage, text: &Rope) -> Option<Arc<Snapshot>> {
    use LapceLanguage::*;
    let (lexer, keywords): (&str, &[&str]) = match language {
        C | Cpp => (
            "cpp",
            &[
                "alignas alignof asm auto bool break case catch char class const constexpr continue default delete do double else enum explicit export extern false float for friend if inline int long namespace new noexcept nullptr operator private protected public register return short signed sizeof static struct switch template this throw true try typedef typename union unsigned using virtual void volatile wchar_t while",
            ],
        ),
        Javascript | Typescript => (
            "cpp",
            &[
                "async await break case catch class const continue debugger default delete do else enum export extends false finally for from function if implements import in instanceof interface let new null of package private protected public return static super switch this throw true try typeof var void while with yield",
            ],
        ),
        Csharp | Java => (
            "cpp",
            &[
                "abstract as base bool boolean break byte case catch char class const continue decimal default delegate do double else enum event extends false final finally float for foreach if implements import in int interface internal is long namespace native new null object operator out override package private protected public readonly ref return sealed short sizeof static string struct super switch synchronized this throw throws true try typeof uint ulong unsafe using var virtual void volatile while",
            ],
        ),
        Python => (
            "python",
            &[
                "False None True and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield",
                "abs all any bool bytes dict enumerate float int len list map max min object open print range repr set str sum super tuple type zip",
            ],
        ),
        Html | Php => (
            "hypertext",
            &[
                "a body div head html input link meta p script span style title",
                "async await const function let new return var",
                "",
                "",
                "abstract as break case catch class const continue default do echo else elseif empty endfor endforeach endif extends false final finally for foreach function global if implements include include_once interface isset list namespace new null private protected public require require_once return static switch throw trait true try unset use var while yield",
            ],
        ),
        Css => ("css", &[]),
        Json | Json5 => ("json", &["false null true"]),
        Bash => (
            "bash",
            &[
                "case do done elif else esac fi for function if in select then time until while",
            ],
        ),
        Rust => (
            "rust",
            &[
                "as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while",
            ],
        ),
        Sql => (
            "sql",
            &[
                "alter and as asc begin between by case create delete desc distinct drop else end exists from group having in index inner insert into is join left like limit not null on or order outer primary references right select set table then union unique update values when where",
            ],
        ),
        Toml => ("toml", &[]),
        Yaml => ("yaml", &[]),
        Xml => ("xml", &[]),
        _ => return None,
    };
    match Snapshot::lex(lexer, &text.to_string(), keywords) {
        Ok(snapshot) => Some(Arc::new(snapshot)),
        Err(error) => {
            tracing::warn!(?error, lexer, "Lexilla highlighting failed");
            None
        }
    }
}

pub fn line_styles(snapshot: &Snapshot, text: &Rope, line: usize) -> Vec<LineStyle> {
    let last = text.line_of_offset(text.len());
    if line > last || snapshot.len() != text.len() {
        return Vec::new();
    }
    let start = text.offset_of_line(line);
    let end = if line == last {
        text.len()
    } else {
        text.offset_of_line(line + 1)
    };
    snapshot
        .runs(start..end)
        .filter_map(|(range, style)| {
            let scope = snapshot.scope(style)?;
            Some(LineStyle {
                start: range.start - start,
                end: range.end - start,
                style: Style {
                    fg_color: Some(scope.to_owned()),
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn line_conversion_uses_theme_scopes_and_utf8_byte_offsets() {
        let text = Rope::from("/* λ\n🙂 */\r\nint value = 42;\n");
        let snapshot = highlight(LapceLanguage::Cpp, &text).unwrap();
        let first = line_styles(&snapshot, &text, 0);
        assert!(
            first
                .iter()
                .any(|s| s.style.fg_color.as_deref() == Some("comment"))
        );
        let code = line_styles(&snapshot, &text, 2);
        assert!(code.iter().any(|s| s.start == 0
            && s.end == 3
            && s.style.fg_color.as_deref() == Some("keyword")));
        assert!(
            code.iter()
                .any(|s| s.style.fg_color.as_deref() == Some("constant"))
        );
        assert!(line_styles(&snapshot, &text, 999).is_empty());
    }
    #[test]
    fn configured_catalogue_modes_are_available() {
        use LapceLanguage::*;
        for language in [
            C, Cpp, Javascript, Typescript, Csharp, Java, Python, Html, Php, Css,
            Json, Json5, Bash, Rust, Sql, Toml, Yaml, Xml,
        ] {
            assert!(
                highlight(language, &Rope::from("text\n")).is_some(),
                "{language:?}"
            );
        }
    }
    #[test]
    fn syntax_worker_publishes_only_lexilla_and_rejects_cancelled_result() {
        let mut syntax = crate::syntax::Syntax::from_language(LapceLanguage::Cpp);
        syntax.parse(7, Rope::from("int value = 42;"), None);
        assert_eq!(syntax.rev, 7);
        assert!(syntax.layers.is_none() && syntax.styles.is_none());
        assert!(syntax.lexilla.is_some());
        syntax
            .cancel_flag
            .store(1, std::sync::atomic::Ordering::Release);
        syntax.parse(8, Rope::from("changed"), None);
        assert_eq!(syntax.rev, 7);
    }
    #[test]
    fn syntax_worker_publishes_local_folds_and_rejects_cancelled_folds() {
        let mut syntax = crate::syntax::Syntax::from_language(LapceLanguage::Php);
        syntax.parse(
            9,
            Rope::from("<?php\nfunction f() {\n    run();\n}\n"),
            None,
        );
        assert_eq!(
            syntax.folding_ranges,
            vec![crate::folding::FoldRange { start: 1, end: 3 }]
        );
        assert!(syntax.layers.is_none());
        syntax
            .cancel_flag
            .store(1, std::sync::atomic::Ordering::Release);
        syntax.parse(10, Rope::from("flat"), None);
        assert_eq!(syntax.rev, 9);
        assert_eq!(syntax.folding_ranges.len(), 1);
    }
    #[test]
    fn plaintext_also_gets_local_folding() {
        let mut syntax = crate::syntax::Syntax::plaintext();
        syntax.parse(1, Rope::from("heading\n    body\nnext"), None);
        assert!(syntax.lexilla.is_none());
        assert_eq!(
            syntax.folding_ranges,
            vec![crate::folding::FoldRange { start: 0, end: 2 }]
        );
        syntax.parse(2, Rope::from("flat"), None);
        assert!(syntax.folding_ranges.is_empty());
    }
    #[test]
    fn unknown_language_is_plain_text() {
        assert!(highlight(LapceLanguage::PlainText, &Rope::from("hello")).is_none());
    }
}
