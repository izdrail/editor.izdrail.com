//! Local outline for Blade views, used by the Document Symbol panel when no
//! language server answers (the panel is otherwise blank). Static line scan:
//! `@section`, `@push`/`@prepend`, `@component`, `@livewire` and `@php`
//! blocks. Comments (`{{-- --}}`) are skipped; `@php(...)` inline calls are
//! not blocks and are left out.
use lsp_types::{DocumentSymbol, Position, Range, SymbolKind};

const MAX_BYTES: usize = 2_000_000;
const MAX_SYMBOLS: usize = 500;

pub fn is_blade_path(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map_or(false, |n| n.ends_with(".blade.php"))
}

/// First quoted string inside the first `(` of `rest`.
fn first_arg(rest: &str) -> Option<String> {
    let open = rest.find('(')?;
    let after = &rest[open + 1..];
    let q = after.find(['\'', '"'])?;
    let quote = after.as_bytes()[q] as char;
    let end = after[q + 1..].find(quote)?;
    let s = &after[q + 1..q + 1 + end];
    (!s.is_empty()).then(|| s.chars().take(80).collect())
}

fn closer_for(kind: &str) -> &'static [&'static str] {
    match kind {
        "section" => &["@endsection", "@show", "@stop", "@append", "@overwrite"],
        "push" | "prepend" => &["@endpush", "@endprepend"],
        "component" => &["@endcomponent"],
        "php" => &["@endphp"],
        _ => &[],
    }
}

#[allow(deprecated)]
fn symbol(name: String, detail: Option<String>, kind: SymbolKind, start: u32, end: u32, width: u32) -> DocumentSymbol {
    let range = Range::new(Position::new(start, 0), Position::new(end, width));
    DocumentSymbol {
        name,
        detail,
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range: Range::new(Position::new(start, 0), Position::new(start, width)),
        children: None,
    }
}

pub fn outline(text: &str) -> Vec<DocumentSymbol> {
    if text.len() > MAX_BYTES {
        return Vec::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut in_comment = false;
    let mut i = 0;
    while i < lines.len() && out.len() < MAX_SYMBOLS {
        let line = lines[i];
        let t = line.trim_start();
        if in_comment {
            if t.contains("--}}") {
                in_comment = false;
            }
            i += 1;
            continue;
        }
        if t.starts_with("{{--") {
            in_comment = !t.contains("--}}");
            i += 1;
            continue;
        }
        let width = line.chars().count() as u32;
        let find_end = |from: usize, kind: &str| -> usize {
            let closers = closer_for(kind);
            lines[from + 1..]
                .iter()
                .position(|l| {
                    let l = l.trim_start();
                    closers.iter().any(|c| l.starts_with(c))
                })
                .map_or(from, |p| from + 1 + p)
        };
        for kind in ["section", "push", "prepend", "component", "livewire"] {
            let tag = format!("@{kind}");
            let Some(rest) = t.strip_prefix(tag.as_str()) else { continue };
            // The directive name must end here: `@sectionMissing` is not one.
            if rest.chars().next().map_or(false, |c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let Some(arg) = first_arg(rest) else { break };
            // `@section('x', 'inline')` has no closing directive.
            let inline = kind == "section" && rest.matches(',').count() > 0;
            let end = if inline || kind == "livewire" { i } else { find_end(i, kind) };
            let sym_kind = match kind {
                "section" => SymbolKind::MODULE,
                "push" | "prepend" => SymbolKind::EVENT,
                _ => SymbolKind::CLASS,
            };
            out.push(symbol(format!("@{kind} {arg}"), None, sym_kind, i as u32, end as u32, width));
            break;
        }
        // A bare `@php` opens a block; `@php(...)` is inline.
        if t.trim_end() == "@php" {
            let end = find_end(i, "php");
            let detail = lines
                .get(i + 1)
                .map(|l| l.trim().chars().take(60).collect::<String>())
                .filter(|s| !s.is_empty() && !s.starts_with("@endphp"));
            out.push(symbol("@php".to_string(), detail, SymbolKind::FUNCTION, i as u32, end as u32, width));
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: &str = "@extends('layouts.app')
{{-- @section('ghost') --}}
@section('content')
@php
    $n = 1;
@endphp
@livewire('counter')
@component('alert')
x
@endcomponent
@endsection
@push('scripts')
@endpush
@php($inline = 1)
@sectionMissing('x')
@section('title', 'Home')
";

    fn names(v: &[DocumentSymbol]) -> Vec<&str> {
        v.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn lists_directives_in_order_and_skips_comments_and_inline() {
        let o = outline(V);
        assert_eq!(
            names(&o),
            [
                "@section content",
                "@php",
                "@livewire counter",
                "@component alert",
                "@push scripts",
                "@section title"
            ]
        );
    }

    #[test]
    fn ranges_cover_the_block() {
        let o = outline(V);
        assert_eq!((o[0].range.start.line, o[0].range.end.line), (2, 10));
        assert_eq!((o[1].range.start.line, o[1].range.end.line), (3, 5));
        assert_eq!(o[1].detail.as_deref(), Some("$n = 1;"));
        // inline @section has a single-line range
        assert_eq!((o[5].range.start.line, o[5].range.end.line), (15, 15));
    }

    #[test]
    fn multiline_comment_and_blade_path() {
        let o = outline("{{--\n@section('x')\n--}}\n@section('y')\n@endsection");
        assert_eq!(names(&o), ["@section y"]);
        assert!(is_blade_path(std::path::Path::new("a/b/show.blade.php")));
        assert!(!is_blade_path(std::path::Path::new("a/b/show.php")));
    }

    #[test]
    fn oversized_input_is_ignored() {
        assert!(outline(&"@php\n".repeat(500_000)).is_empty());
    }
}
