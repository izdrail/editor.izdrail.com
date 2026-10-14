//! "Sort Tailwind Classes": reorders the utilities in static `class="..."` /
//! `className="..."` attributes into Tailwind's usual layout order (layout,
//! spacing, sizing, typography, backgrounds, borders, effects, transitions),
//! with variant classes (`hover:`, `dark:`, `md:`) after the plain ones.
//!
//! This is an approximation of Tailwind v3's order, not a port of the real
//! one: groups follow the generated CSS order, but utilities inside a group
//! keep their original relative order, and unknown (custom) classes come
//! first, in their original order. Dynamic values (`{{ }}`, `{!! !!}`, `<?php`,
//! `@class`, `${}`, `{...}`) and values with a line break are never touched.
use std::ops::Range;

const MAX_BYTES: usize = 1024 * 1024;

/// (group rank, prefixes). A class matches when its utility name equals a
/// prefix or starts with `prefix-`; entries ending in `-` match by prefix only.
const GROUPS: &[(u32, &[&str])] = &[
    (1, &["container"]),
    (2, &["sr-only", "not-sr-only", "pointer-events", "visible", "invisible", "collapse"]),
    (3, &["static", "fixed", "absolute", "relative", "sticky"]),
    (4, &["inset", "inset-x", "inset-y", "top", "right", "bottom", "left", "start", "end", "z"]),
    (5, &["isolate", "order", "col", "row", "float", "clear"]),
    (6, &["m", "mx", "my", "mt", "mr", "mb", "ml", "ms", "me"]),
    (7, &["box", "line-clamp"]),
    (
        8,
        &[
            "block", "inline-block", "inline", "flex", "inline-flex", "table", "inline-table",
            "grid", "inline-grid", "contents", "hidden", "flow-root", "list-item",
        ],
    ),
    (9, &["aspect", "size", "h", "min-h", "max-h", "w", "min-w", "max-w"]),
    (
        10,
        &[
            "shrink", "grow", "basis", "border-collapse", "border-spacing", "origin",
            "translate", "rotate", "skew", "scale", "transform", "animate", "cursor",
            "touch", "select", "resize", "snap", "scroll", "list", "appearance",
        ],
    ),
    (
        11,
        &[
            "flex-row", "flex-col", "flex-wrap", "flex-nowrap", "flex-row-reverse",
            "flex-col-reverse", "flex-wrap-reverse", "grid-cols", "grid-rows", "grid-flow",
            "auto-cols", "auto-rows", "place", "content", "items", "justify", "self", "gap",
            "gap-x", "gap-y", "space-x", "space-y",
        ],
    ),
    (12, &["overflow", "overflow-x", "overflow-y", "overscroll", "truncate", "text-ellipsis", "text-clip", "whitespace", "break"]),
    (13, &["rounded", "border", "divide", "outline", "ring"]),
    (14, &["bg", "from", "via", "to"]),
    (15, &["fill", "stroke", "object"]),
    (16, &["p", "px", "py", "pt", "pr", "pb", "pl", "ps", "pe"]),
    (
        17,
        &[
            "text", "font", "leading", "tracking", "indent", "align", "uppercase",
            "lowercase", "capitalize", "normal-case", "italic", "not-italic", "underline",
            "overline", "line-through", "no-underline", "antialiased", "subpixel-antialiased",
            "decoration", "underline-offset", "tabular-nums", "lining-nums", "oldstyle-nums",
            "ordinal", "slashed-zero", "diagonal-fractions", "stacked-fractions",
        ],
    ),
    (18, &["opacity", "mix-blend", "bg-blend", "shadow", "ring-offset", "filter", "blur", "brightness", "contrast", "drop-shadow", "grayscale", "hue-rotate", "invert", "saturate", "sepia", "backdrop"]),
    (19, &["transition", "duration", "ease", "delay", "will-change"]),
];

const PSEUDO: &[&str] = &[
    "first", "last", "only", "odd", "even", "first-of-type", "last-of-type", "visited",
    "target", "open", "default", "checked", "indeterminate", "required", "valid", "invalid",
    "hover", "focus-within", "focus", "focus-visible", "active", "enabled", "disabled",
    "placeholder", "file", "marker", "selection", "before", "after", "group-hover",
    "group-focus", "peer-hover", "peer-focus", "empty", "motion-safe", "motion-reduce",
];
const SCREENS: &[&str] = &["sm", "md", "lg", "xl", "2xl"];

fn variant_rank(v: &str) -> u32 {
    if let Some(i) = SCREENS.iter().position(|s| *s == v) {
        return 100 + i as u32;
    }
    if v == "dark" {
        return 50;
    }
    if PSEUDO.contains(&v) || v.starts_with("group-") || v.starts_with("peer-") || v.starts_with('[') {
        return 10;
    }
    if v.starts_with("min-") || v.starts_with("max-") {
        return 120;
    }
    20
}

/// Splits `md:hover:bg-red-500` into its variants and the utility. A `:`
/// inside `[...]` (arbitrary values) does not split.
fn split_variants(class: &str) -> (Vec<&str>, &str) {
    let (mut depth, mut start) = (0i32, 0usize);
    let mut variants = vec![];
    for (i, c) in class.char_indices() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ':' if depth == 0 => {
                variants.push(&class[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    (variants, &class[start..])
}

fn group_of(util: &str) -> Option<u32> {
    let util = util.trim_start_matches(['!', '-']);
    let util = util.split_once('/').map_or(util, |(a, _)| a);
    // Longest-prefix match so `min-h-0` is sizing, not margin or `min`.
    let mut best: Option<(usize, u32)> = None;
    for (rank, names) in GROUPS {
        for n in *names {
            let hit = util == *n || (util.starts_with(n) && util[n.len()..].starts_with('-'));
            if hit && best.is_none_or(|(l, _)| n.len() > l) {
                best = Some((n.len(), *rank));
            }
        }
    }
    best.map(|(_, r)| r)
}

/// (variant rank, group rank). Unknown utilities rank 0 and sort first.
fn rank(class: &str) -> (u32, u32) {
    let (variants, util) = split_variants(class);
    let Some(group) = group_of(util) else {
        return (0, 0);
    };
    let v = variants.iter().map(|v| variant_rank(v)).max().unwrap_or(0);
    (v + variants.len() as u32, group)
}

/// The class string sorted, or None when nothing would change.
pub fn sort_classes(value: &str) -> Option<String> {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    if tokens.len() < 2 {
        return None;
    }
    let mut keyed: Vec<((u32, u32), usize, &str)> =
        tokens.iter().enumerate().map(|(i, t)| (rank(t), i, *t)).collect();
    // Unknown classes (rank 0,0) stay first only if they were not variants.
    keyed.sort_by_key(|(r, i, _)| (*r, *i));
    let sorted: Vec<&str> = keyed.into_iter().map(|(_, _, t)| t).collect();
    let out = sorted.join(" ");
    (sorted != tokens || out != value.trim() ).then_some(out).filter(|o| o != value)
}

fn is_static(value: &str) -> bool {
    !value.contains(['{', '}', '<', '>', '@', '$', '\n', '\r', '`'])
}

/// Edits (byte range of the attribute value, new value) for every static
/// `class="..."`/`className="..."` in `text[range]`.
pub fn sort_attributes(text: &str, range: Range<usize>) -> Vec<(Range<usize>, String)> {
    let mut out = vec![];
    if text.len() > MAX_BYTES || range.end > text.len() {
        return out;
    }
    let b = text.as_bytes();
    for name in ["className=", "class="] {
        let mut from = range.start;
        while let Some(p) = text.get(from..range.end).and_then(|s| s.find(name)) {
            let at = from + p;
            from = at + name.len();
            let boundary = at == 0 || b[at - 1].is_ascii_whitespace();
            if !boundary || (name == "class=" && at >= 4 && &b[at - 4..at] == b"Name") {
                continue;
            }
            let Some(&q) = b.get(from) else { break };
            if q != b'"' && q != b'\'' {
                continue;
            }
            let vstart = from + 1;
            let Some(len) = text.get(vstart..range.end).and_then(|s| s.find(q as char)) else {
                continue;
            };
            let value = &text[vstart..vstart + len];
            if !is_static(value) {
                continue;
            }
            if let Some(new) = sort_classes(value) {
                out.push((vstart..vstart + len, new));
            }
            from = vstart + len;
        }
    }
    out.sort_by_key(|(r, _)| r.start);
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_by_group() {
        assert_eq!(
            sort_classes("text-sm p-4 bg-white flex mt-2 rounded").as_deref(),
            Some("mt-2 flex rounded bg-white p-4 text-sm")
        );
    }

    #[test]
    fn variants_follow_base_classes() {
        assert_eq!(
            sort_classes("md:flex hover:bg-blue-500 p-4 dark:bg-black lg:p-8 bg-white").as_deref(),
            Some("bg-white p-4 hover:bg-blue-500 dark:bg-black md:flex lg:p-8")
        );
    }

    #[test]
    fn unknown_classes_stay_first_in_order() {
        assert_eq!(
            sort_classes("p-4 btn-primary my-widget flex").as_deref(),
            Some("btn-primary my-widget flex p-4")
        );
    }

    #[test]
    fn stable_inside_a_group_and_idempotent() {
        assert_eq!(sort_classes("px-2 py-1 p-3"), None);
        let once = sort_classes("p-4 flex").unwrap();
        assert_eq!(sort_classes(&once), None);
    }

    #[test]
    fn negative_important_arbitrary_and_prefix_edge_cases() {
        assert_eq!(sort_classes("p-4 -mt-2").as_deref(), Some("-mt-2 p-4"));
        assert_eq!(sort_classes("p-4 !mt-2").as_deref(), Some("!mt-2 p-4"));
        assert_eq!(sort_classes("p-4 min-h-0").as_deref(), Some("min-h-0 p-4"));
        assert_eq!(sort_classes("text-red-500 w-[calc(100%-2px)] flex").as_deref(), Some("flex w-[calc(100%-2px)] text-red-500"));
        assert_eq!(sort_classes("p-4 [&>*]:flex mt-1").as_deref(), Some("mt-1 p-4 [&>*]:flex"));
        assert_eq!(sort_classes("one"), None);
        assert_eq!(sort_classes(""), None);
    }

    #[test]
    fn rewrites_static_attributes_only() {
        let t = r#"<div class="p-4 flex" className='text-sm mt-2'><i class="{{ $c }} p-4 flex"></i><b data-class="p-4 flex"></b><u class="a @if($x) p-4 flex @endif"></u></div>"#;
        let edits = sort_attributes(t, 0..t.len());
        let mut out = t.to_string();
        for (r, s) in edits.iter().rev() {
            out.replace_range(r.clone(), s);
        }
        assert!(out.contains(r#"class="flex p-4""#), "{out}");
        assert!(out.contains("className='mt-2 text-sm'"), "{out}");
        assert!(out.contains(r#"class="{{ $c }} p-4 flex""#), "{out}");
        assert!(out.contains(r#"data-class="p-4 flex""#), "{out}");
        assert!(out.contains(r#"class="a @if($x) p-4 flex @endif""#), "{out}");
    }

    #[test]
    fn selection_range_limits_edits() {
        let t = r#"<a class="p-4 flex"></a><a class="p-4 flex"></a>"#;
        let first_end = t.find("</a>").unwrap();
        assert_eq!(sort_attributes(t, 0..first_end).len(), 1);
    }

    #[test]
    fn odd_and_large_inputs() {
        assert!(sort_attributes("class=", 0..6).is_empty());
        assert!(sort_attributes("class=\"", 0..7).is_empty());
        assert!(sort_attributes("é class=\"p-4 flex\"", 0..19).len() <= 1);
        assert!(sort_attributes(&"a".repeat(2 * 1024 * 1024), 0..10).is_empty());
        let many = r#"<i class="p-4 flex"></i>"#.repeat(20_000);
        assert_eq!(sort_attributes(&many, 0..many.len()).len(), 20_000);
        let long = format!("<i class=\"{}\">", "p-4 flex ".repeat(50_000));
        let _ = sort_attributes(&long, 0..long.len());
    }
}
