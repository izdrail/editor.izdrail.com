//! Adjacent structural matching from an already-current lexer snapshot.
//! No parsing or full-buffer copy runs on the input/paint path.
use lapce_xi_rope::{Cursor, Rope};
pub use lexilla_rs::Snapshot;

const SCAN_BYTES: usize = 64 * 1024;
const MAX_DEPTH: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdjacentMatch {
    /// Missing/stale lexical evidence, unknown scope, or scan budget exhausted.
    Unknown,
    NotAdjacent,
    /// A bracket in a string/comment is deliberately not structural.
    Ignored,
    Matched { at: usize, other: usize },
    Unmatched { at: usize },
}

fn partner(c: char) -> Option<char> {
    Some(match c { '(' => ')', '[' => ']', '{' => '}', ')' => '(', ']' => '[', '}' => '{', _ => return None })
}

pub fn adjacent_match(text: &Rope, snapshot: Option<&Snapshot>, snapshot_rev: u64,
    buffer_rev: u64, offset: usize) -> AdjacentMatch {
    let Some(snapshot) = snapshot.filter(|s| snapshot_rev == buffer_rev && s.len() == text.len()) else {
        return AdjacentMatch::Unknown;
    };
    if offset > text.len() || text.at_or_prev_codepoint_boundary(offset) != Some(offset) {
        return AdjacentMatch::Unknown;
    }
    let mut cursor = Cursor::new(text, offset);
    let candidate = cursor.peek_next_codepoint().filter(|c| partner(*c).is_some()).map(|c| (offset, c))
        .or_else(|| {
            let c = cursor.prev_codepoint()?;
            partner(c).map(|_| (cursor.pos(), c))
        });
    let Some((at, c)) = candidate else { return AdjacentMatch::NotAdjacent; };
    let scope = |at| snapshot.scope(snapshot.styles()[at]);
    match scope(at) {
        Some("comment" | "string") => return AdjacentMatch::Ignored,
        Some("operator") => (),
        _ => return AdjacentMatch::Unknown,
    }
    let forward = matches!(c, '(' | '[' | '{');
    let embed = snapshot.embed_at(at);
    let mut stack = vec![c];
    let mut cursor = Cursor::new(text, at);
    if forward { cursor.next_codepoint(); }
    loop {
        let position = cursor.pos();
        if position.abs_diff(at) > SCAN_BYTES { return AdjacentMatch::Unknown; }
        let next = if forward { cursor.next_codepoint() } else { cursor.prev_codepoint() };
        let Some(next) = next else { return AdjacentMatch::Unmatched { at }; };
        let position = if forward { position } else { cursor.pos() };
        if position.abs_diff(at) > SCAN_BYTES { return AdjacentMatch::Unknown; }
        // Crossing embedded-language boundaries is not evidence of a missing
        // partner. Leave it unknown rather than paint a false syntax error.
        if snapshot.embed_at(position) != embed { return AdjacentMatch::Unknown; }
        if partner(next).is_none() { continue; }
        match scope(position) {
            Some("comment" | "string") => continue,
            Some("operator") => (),
            _ => return AdjacentMatch::Unknown,
        }
        let opening = matches!(next, '(' | '[' | '{');
        if opening == forward {
            if stack.len() == MAX_DEPTH { return AdjacentMatch::Unknown; }
            stack.push(next);
        } else {
            if stack.pop().and_then(partner) != Some(next) { return AdjacentMatch::Unknown; }
            if stack.is_empty() { return AdjacentMatch::Matched { at, other: position }; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{language::LapceLanguage, lexilla};

    #[test]
    fn lexical_brackets_skip_strings_comments_and_preserve_adjacency() {
        for (language, source) in [
            (LapceLanguage::Php, "<?php\n{ $x = '}'; /* } */ call([]); }\n"),
            (LapceLanguage::Python, "x = (')', # )\n [])\n"),
            (LapceLanguage::Rust, "fn main() { let s = \"}\"; /* } */ call([]); }\n"),
            (LapceLanguage::Javascript, "function f() { let s = '}'; /* } */ call([]); }\n"),
            (LapceLanguage::Typescript, "function f(): void { let s = '}'; /* } */ call([]); }\n"),
            (LapceLanguage::Css, "a { content: '}'; /* } */ }\n"),
        ] {
            let text = Rope::from(source);
            let snapshot = lexilla::highlight(language, &text).unwrap();
            let at = source.find(if language == LapceLanguage::Python { '(' } else { '{' }).unwrap();
            let other = source.rfind(if language == LapceLanguage::Python { ')' } else { '}' }).unwrap();
            for (offset, a, b) in [(at, at, other), (other, other, at), (other+1, other, at)] {
                assert_eq!(adjacent_match(&text, Some(&snapshot), 4, 4, offset), AdjacentMatch::Matched { at: a, other: b }, "{language:?} at {offset}");
            }
            let quoted = source.find("')'").map(|n|n+1).or_else(||source.find("'}'").map(|n|n+1)).or_else(||source.find("\"}\"").map(|n|n+1)).unwrap();
            assert_eq!(adjacent_match(&text, Some(&snapshot), 4, 4, quoted), AdjacentMatch::Ignored, "{language:?}");
        }
    }

    #[test]
    fn lexical_brackets_unknown_is_not_unmatched() {
        let text = Rope::from("<?php\n(");
        let snapshot = lexilla::highlight(LapceLanguage::Php, &text).unwrap();
        assert_eq!(adjacent_match(&text, Some(&snapshot), 1, 1, 6), AdjacentMatch::Unmatched { at: 6 });
        assert_eq!(adjacent_match(&text, Some(&snapshot), 0, 1, 6), AdjacentMatch::Unknown);
        assert_eq!(adjacent_match(&text, None, 1, 1, 6), AdjacentMatch::Unknown);
        assert_eq!(adjacent_match(&Rope::from("<?php\n()"), Some(&snapshot), 1, 1, 6), AdjacentMatch::Unknown);
        assert_eq!(adjacent_match(&text, Some(&snapshot), 1, 1, usize::MAX), AdjacentMatch::Unknown);
        for source in [format!("<?php ({}", " ".repeat(SCAN_BYTES+1)), format!("<?php {}", "(".repeat(MAX_DEPTH+1))] {
            let text=Rope::from(source.as_str());
            let snapshot=lexilla::highlight(LapceLanguage::Php,&text).unwrap();
            assert_eq!(adjacent_match(&text,Some(&snapshot),1,1,6),AdjacentMatch::Unknown);
        }
    }

    #[test]
    fn lexical_brackets_unsupported_scope_and_embed_boundary_stay_unknown() {
        let text = Rope::from("() ");
        let unknown = Snapshot::from_php_styles("() ", vec![0; 3]).unwrap();
        assert_eq!(adjacent_match(&text, Some(&unknown), 1, 1, 0), AdjacentMatch::Unknown);
        let mut embedded = Snapshot::from_php_styles("() ", vec![11; 3]).unwrap();
        assert_eq!(adjacent_match(&text, Some(&embedded), 1, 1, 0), AdjacentMatch::Matched { at: 0, other: 1 });
        embedded.record_embed(0, 1, lexilla_rs::Embed::Script);
        assert_eq!(adjacent_match(&text, Some(&embedded), 1, 1, 0), AdjacentMatch::Unknown);
        assert_eq!(adjacent_match(&text, Some(&embedded), 1, 1, 1), AdjacentMatch::Unknown);
    }

    #[cfg(feature = "logos-php")]
    #[test]
    fn lexical_brackets_default_logos_import_is_live() {
        let source="<?php { $x = '}'; /* } */ }";
        let text=Rope::from(source);
        let snapshot=crate::lexilla::highlight_profile_backend(LapceLanguage::Php,&text,&crate::project_profile::ProjectProfile::default(),Some("logos")).unwrap();
        assert!(snapshot.lexer().ends_with("logos"));
        assert_eq!(adjacent_match(&text,Some(&snapshot),1,1,6),AdjacentMatch::Matched { at: 6, other: source.len()-1 });
        let quoted=source.find("'}'").unwrap()+1;
        assert_eq!(adjacent_match(&text,Some(&snapshot),1,1,quoted),AdjacentMatch::Ignored);
        assert_eq!(adjacent_match(&text,Some(&snapshot),0,1,6),AdjacentMatch::Unknown);
    }

    #[test]
    fn lexical_brackets_mixed_web_regions_match_locally_and_stop_at_boundaries() {
        for language in [LapceLanguage::Html, LapceLanguage::Blade, LapceLanguage::Vue] {
            #[allow(unused_mut)]
            let mut backends = vec!["lexilla"];
            #[cfg(feature = "logos-php")]
            backends.push("logos");
            for backend in backends {
                let profile = crate::project_profile::ProjectProfile::default();
                let expected_lexer = if language == LapceLanguage::Blade {
                    if backend == "logos" { "blade-logos" } else { "blade" }
                } else { "hypertext" };
                eprintln!("mixed bracket route {language:?}/{backend}: {expected_lexer}");
                for (source, start, end, kind) in [
                    ("<script>function run() { const s = '}'; /* } */ [] }</script>", '{', '}', lexilla_rs::Embed::Script),
                    ("<style>a { content: '}'; /* } */ }</style>", '{', '}', lexilla_rs::Embed::Style),
                ] {
                    let text=Rope::from(source);
                    let snapshot=crate::lexilla::highlight_profile_backend(language,&text,&profile,Some(backend)).unwrap();
                    assert_eq!(snapshot.lexer(),expected_lexer,"{language:?}/{backend} actual route");
                    let at=source.find(start).unwrap();
                    let other=source.rfind(end).unwrap();
                    assert_eq!(snapshot.embed_at(at),Some(kind),"{language:?}/{backend}");
                    assert_eq!(adjacent_match(&text,Some(&snapshot),1,1,at),AdjacentMatch::Matched { at, other },"{language:?}/{backend}");
                    assert_eq!(adjacent_match(&text,Some(&snapshot),1,1,other),AdjacentMatch::Matched { at: other, other: at },"{language:?}/{backend}");
                    let quoted=source.find("'}'").unwrap()+1;
                    assert_eq!(adjacent_match(&text,Some(&snapshot),1,1,quoted),AdjacentMatch::Ignored,"{language:?}/{backend}");
                }
                for (source, kind) in [
                    ("<script>{</script><script>}</script>", lexilla_rs::Embed::Script),
                    ("<style>a {</style><style>}</style>", lexilla_rs::Embed::Style),
                ] {
                    let text=Rope::from(source);
                    let snapshot=crate::lexilla::highlight_profile_backend(language,&text,&profile,Some(backend)).unwrap();
                    assert_eq!(snapshot.lexer(),expected_lexer,"{language:?}/{backend}: {source}");
                    for at in [source.find('{').unwrap(),source.rfind('}').unwrap()] {
                        assert_eq!(snapshot.scope(snapshot.styles()[at]),Some("operator"),"{language:?}/{backend}: {source} at {at}");
                        assert_eq!(snapshot.embed_at(at),Some(kind),"{language:?}/{backend}: {source} at {at}");
                        assert_eq!(adjacent_match(&text,Some(&snapshot),1,1,at),AdjacentMatch::Unknown,"{language:?}/{backend} at {at}");
                    }
                }
                let source="<script>(</script><style>)</style>";
                let text=Rope::from(source);
                let snapshot=crate::lexilla::highlight_profile_backend(language,&text,&profile,Some(backend)).unwrap();
                assert_eq!(snapshot.lexer(),expected_lexer,"{language:?}/{backend}: {source}");
                for (at,kind) in [(source.find('(').unwrap(),lexilla_rs::Embed::Script),(source.find(')').unwrap(),lexilla_rs::Embed::Style)] {
                    assert_eq!(snapshot.scope(snapshot.styles()[at]),Some("operator"),"{language:?}/{backend}: {source} at {at}");
                    assert_eq!(snapshot.embed_at(at),Some(kind),"{language:?}/{backend}: {source} at {at}");
                    assert_eq!(adjacent_match(&text,Some(&snapshot),1,1,at),AdjacentMatch::Unknown,"{language:?}/{backend}: {source} at {at}");
                }
                let host_sources: &[&str] = match language {
                    LapceLanguage::Blade => &["<p>{</p>", "{{ x }}", "{{ x }", "{!! $x !!}", "{{-- x --}}", "@{{ x }}", "@php $x = '{'; @endphp", "@if('(') text @endif"],
                    LapceLanguage::Vue => &["<p>{</p>", "{{ x }}", "{{ x }", r#"<p v-if="x === '('">text</p>"#],
                    _ => &["<p>{</p>"],
                };
                for source in host_sources {
                    let text=Rope::from(*source);
                    let snapshot=crate::lexilla::highlight_profile_backend(language,&text,&profile,Some(backend)).unwrap();
                    assert_eq!(snapshot.lexer(),expected_lexer,"{language:?}/{backend}: {source}");
                    let offsets: Vec<_> = if source.starts_with("@if") || source.starts_with("@php") || source.contains("v-if") {
                        vec![source.find('\'').unwrap()+1]
                    } else { source.char_indices().filter_map(|(at,c)| matches!(c,'{'|'}').then_some(at)).collect() };
                    for at in offsets {
                        if language == LapceLanguage::Vue && source.contains('{') {
                            assert_ne!(snapshot.scope(snapshot.styles()[at]),Some("operator"),"Vue host scope {backend}: {source} at {at}");
                        }
                        assert!(matches!(adjacent_match(&text,Some(&snapshot),1,1,at),AdjacentMatch::Unknown|AdjacentMatch::Ignored),"host delimiter {language:?}/{backend}: {source} at {at}");
                    }
                }
            }
        }
    }

    #[test]
    fn lexical_brackets_backward_eof_and_next_precedence() {
        for (source, offset, expected) in [
            ("<?php )", 6, AdjacentMatch::Unmatched { at: 6 }),
            ("<?php (", 7, AdjacentMatch::Unmatched { at: 6 }),
            ("<?php )( ", 7, AdjacentMatch::Unmatched { at: 7 }),
        ] {
            let text=Rope::from(source);
            let snapshot=lexilla::highlight(LapceLanguage::Php,&text).unwrap();
            assert_eq!(adjacent_match(&text,Some(&snapshot),1,1,offset),expected);
        }
    }

    #[test]
    #[ignore = "hosted timing evidence"]
    fn lexical_brackets_64k_unmatched_baseline() {
        let source=format!("({}", " ".repeat(SCAN_BYTES-1));
        let text=Rope::from(source.as_str());
        let snapshot=Snapshot::from_php_styles(&source,vec![11;source.len()]).unwrap();
        let started=std::time::Instant::now();
        for _ in 0..100 {
            assert_eq!(std::hint::black_box(adjacent_match(&text,Some(&snapshot),1,1,0)),AdjacentMatch::Unmatched { at: 0 });
        }
        let elapsed = started.elapsed();
        eprintln!("64KiB unmatched single-line scan: 100 iterations {:?}; per scan {:?}; debug_assertions={}; embed regions 0; target <1ms/scan, report-only", elapsed, elapsed / 100, cfg!(debug_assertions));
    }

    #[test]
    fn lexical_brackets_typed_nesting_and_unicode() {
        for (source, offset, expected) in [
            ("<?php λ([)]", 8, AdjacentMatch::Unknown),
            ("<?php λ([])", 8, AdjacentMatch::Matched { at: 8, other: 11 }),
            ("<?php λ([])", 7, AdjacentMatch::Unknown),
            ("<?php λ x", 9, AdjacentMatch::NotAdjacent),
        ] {
            let text=Rope::from(source);
            let snapshot=lexilla::highlight(LapceLanguage::Php,&text).unwrap();
            assert_eq!(adjacent_match(&text,Some(&snapshot),1,1,offset),expected);
        }
    }
}
