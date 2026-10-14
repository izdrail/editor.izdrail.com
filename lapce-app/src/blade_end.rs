//! Blade: pressing Enter after a block directive (`@if($x)`, `@foreach(...)`,
//! `@section('a')`, a bare `@php`, ...) adds the matching `@endif` /
//! `@endforeach` / ... below the new line, unless the block is already closed
//! further down. Whole-line directives only: `@if($x) a @endif` and
//! `@php($x = 1)` are inline and left alone.
const MAX_SCAN: usize = 200_000;

/// (directive, closing directive). `if`-like directives all end with `@endif`.
const BLOCKS: [(&str, &str); 33] = [
    ("if", "@endif"),
    ("unless", "@endunless"),
    ("foreach", "@endforeach"),
    ("forelse", "@endforelse"),
    ("for", "@endfor"),
    ("while", "@endwhile"),
    ("isset", "@endisset"),
    ("empty", "@endempty"),
    ("auth", "@endauth"),
    ("guest", "@endguest"),
    ("can", "@endcan"),
    ("cannot", "@endcannot"),
    ("canany", "@endcanany"),
    ("env", "@endenv"),
    ("production", "@endproduction"),
    ("error", "@enderror"),
    ("hasSection", "@endif"),
    ("sectionMissing", "@endif"),
    ("once", "@endonce"),
    ("verbatim", "@endverbatim"),
    ("push", "@endpush"),
    ("prepend", "@endprepend"),
    ("pushOnce", "@endPushOnce"),
    ("prependOnce", "@endPrependOnce"),
    ("pushIf", "@endPushIf"),
    ("fragment", "@endfragment"),
    ("session", "@endsession"),
    ("switch", "@endswitch"),
    ("component", "@endcomponent"),
    ("slot", "@endslot"),
    ("section", "@endsection"),
    ("php", "@endphp"),
    ("teleport", "@endteleport"),
];

/// Whether `(...)` at the start of `rest` is balanced and ends the line.
/// Returns the number of top-level comma separated arguments.
fn whole_args(rest: &str) -> Option<usize> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Some(0);
    }
    if !rest.starts_with('(') {
        return None;
    }
    let (mut depth, mut args, mut quote) = (0i32, 1usize, None::<char>);
    let mut prev = ' ';
    for (i, c) in rest.char_indices() {
        if let Some(q) = quote {
            if c == q && prev != '\\' {
                quote = None;
            }
            prev = c;
            continue;
        }
        match c {
            '\'' | '"' => quote = Some(c),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return (i + 1 == rest.len()).then_some(args);
                }
                if depth < 0 {
                    return None;
                }
            }
            ',' if depth == 1 => args += 1,
            _ => {}
        }
        prev = c;
    }
    None
}

fn directives(text: &str) -> impl Iterator<Item = &str> {
    let bytes = text.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < bytes.len() {
            if bytes[i] == b'@' {
                let escaped = i > 0 && bytes[i - 1] == b'@';
                let start = i + 1;
                let mut end = start;
                while end < bytes.len() && bytes[end].is_ascii_alphanumeric() {
                    end += 1;
                }
                i = end.max(i + 1);
                if !escaped && end > start {
                    return Some(&text[start..end]);
                }
            } else {
                i += 1;
            }
        }
        None
    })
}

/// The closing directive to add after `prev_line`, or None. `following` is the
/// text after the new line (used to see whether the block is closed already).
pub fn end_directive(prev_line: &str, following: &str) -> Option<&'static str> {
    let line = prev_line.trim();
    let body = line.strip_prefix('@')?;
    let name_len = body.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(body.len());
    let name = &body[..name_len];
    let (_, end) = *BLOCKS.iter().find(|(n, _)| *n == name)?;
    let args = whole_args(&body[name_len..])?;
    let has_parens = !body[name_len..].trim().is_empty();
    match name {
        // `@php($x)` is inline; `@php` alone opens a block.
        "php" if has_parens => return None,
        // `@section('a', 'b')` is inline; `@empty` alone is a @forelse branch.
        "section" if args != 1 => return None,
        "empty" if !has_parens => return None,
        // `@slot` is a block only for components using it with a name.
        "slot" if args == 0 => return None,
        _ => {}
    }
    let mut cut = following.len().min(MAX_SCAN);
    while !following.is_char_boundary(cut) {
        cut -= 1;
    }
    let end_name = &end[1..];
    let family_end = end_name;
    let mut depth = 1i32;
    for d in directives(&following[..cut]) {
        let opens = if end == "@endif" {
            matches!(d, "if" | "hasSection" | "sectionMissing")
        } else {
            d == name
        };
        if opens {
            depth += 1;
        } else if d == family_end {
            depth -= 1;
            if depth == 0 {
                return None;
            }
        }
    }
    Some(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(line: &str) -> Option<&'static str> {
        end_directive(line, "")
    }

    #[test]
    fn common_blocks() {
        assert_eq!(e("@if($user)"), Some("@endif"));
        assert_eq!(e("    @foreach ($users as $user)"), Some("@endforeach"));
        assert_eq!(e("@forelse($a as $b)"), Some("@endforelse"));
        assert_eq!(e("@auth"), Some("@endauth"));
        assert_eq!(e("@auth('admin')"), Some("@endauth"));
        assert_eq!(e("@section('content')"), Some("@endsection"));
        assert_eq!(e("@push('scripts')"), Some("@endpush"));
        assert_eq!(e("@php"), Some("@endphp"));
        assert_eq!(e("@error('email')"), Some("@enderror"));
        assert_eq!(e("@hasSection('x')"), Some("@endif"));
        assert_eq!(e("@can('edit', $post)"), Some("@endcan"));
    }

    #[test]
    fn nested_parens_and_strings() {
        assert_eq!(e("@if(count($a) > 0 && in_array(')', $b))"), Some("@endif"));
        assert_eq!(e("@if($a === \"(\")"), Some("@endif"));
    }

    #[test]
    fn inline_and_non_blocks_are_skipped() {
        assert_eq!(e("@if($a) yes @endif"), None);
        assert_eq!(e("@php($x = 1)"), None);
        assert_eq!(e("@section('title', 'Home')"), None);
        assert_eq!(e("@empty"), None);
        assert_eq!(e("@else"), None);
        assert_eq!(e("@elseif($x)"), None);
        assert_eq!(e("@include('x')"), None);
        assert_eq!(e("@extends('layout')"), None);
        assert_eq!(e("@yield('a')"), None);
        assert_eq!(e("@if($a"), None);
        assert_eq!(e("@if($a)) x"), None);
        assert_eq!(e("<div @if($a)"), None);
        assert_eq!(e("@@if($a)"), None);
        assert_eq!(e("if($a)"), None);
        assert_eq!(e(""), None);
        assert_eq!(e("@"), None);
    }

    #[test]
    fn already_closed_is_skipped() {
        assert_eq!(end_directive("@if($a)", "\n  x\n@endif\n"), None);
        assert_eq!(end_directive("@if($a)", "@if($b)\n@endif\n@endif"), None);
    }

    #[test]
    fn unbalanced_following_still_closes() {
        // the inner block closes itself, ours is still open
        assert_eq!(end_directive("@if($a)", "@if($b)\n@endif\n"), Some("@endif"));
        assert_eq!(end_directive("@foreach($a as $b)", "@endif\n"), Some("@endforeach"));
        assert_eq!(end_directive("@if($a)", "@elseif($b)\n@else\n"), Some("@endif"));
        assert_eq!(end_directive("@if($a)", "@@endif"), Some("@endif"));
    }

    #[test]
    fn hassection_closes_with_endif_family() {
        assert_eq!(end_directive("@hasSection('a')", "@endif"), None);
        assert_eq!(end_directive("@if($a)", "@hasSection('a')\n@endif\n"), Some("@endif"));
    }

    #[test]
    fn huge_and_non_ascii_inputs() {
        let big = "é@endif ".repeat(100_000);
        assert_eq!(end_directive("@if($a)", &big), None);
        let none = "é".repeat(300_000);
        assert_eq!(end_directive("@if($é)", &none), Some("@endif"));
        assert_eq!(e("@if(é"), None);
    }
}
