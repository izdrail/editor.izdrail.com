//! Hover help for Tailwind utility classes in `class="..."` / `className="..."`
//! attributes: hovering `md:hover:p-4` shows `padding: 1rem;` and the variants
//! that apply (`@media (min-width: 768px)`, `:hover`).
//!
//! Covers the Tailwind v3 default theme only (values checked against
//! tailwindcss v3.4.17 `stubs/config.full.js`): spacing utilities (padding,
//! margin, gap, width, height, size, inset), rounded, shadow, text sizes, font
//! weights, display, position, flex, alignment, grid columns/rows and spans,
//! z-index, opacity, tracking, leading, order, max/min width/height and the
//! `space-x`/`divide-x` sibling utilities, plus the default colour palette (`bg-red-500`, `text-gray-700/50`,
//! `border-t-blue-600`) and arbitrary values (`w-[13px]`, `bg-[#1da1f2]`,
//! `text-[length:14px]`). Classes that were renamed in Tailwind v4 say so
//! (`shadow-sm` -> `shadow-xs`). Custom theme values from `tailwind.config.js`
//! and other Tailwind v4 differences are not covered; a class that matches
//! nothing gets no hover. Nothing is executed and no file is read: only the hovered line is looked at.

const MAX_LINE: usize = 4096;

const SPACING: &[(&str, &str)] = &[
    ("px", "1px"), ("0", "0px"), ("0.5", "0.125rem"), ("1", "0.25rem"), ("1.5", "0.375rem"),
    ("2", "0.5rem"), ("2.5", "0.625rem"), ("3", "0.75rem"), ("3.5", "0.875rem"), ("4", "1rem"),
    ("5", "1.25rem"), ("6", "1.5rem"), ("7", "1.75rem"), ("8", "2rem"), ("9", "2.25rem"),
    ("10", "2.5rem"), ("11", "2.75rem"), ("12", "3rem"), ("14", "3.5rem"), ("16", "4rem"),
    ("20", "5rem"), ("24", "6rem"), ("28", "7rem"), ("32", "8rem"), ("36", "9rem"),
    ("40", "10rem"), ("44", "11rem"), ("48", "12rem"), ("52", "13rem"), ("56", "14rem"),
    ("60", "15rem"), ("64", "16rem"), ("72", "18rem"), ("80", "20rem"), ("96", "24rem"),
];

/// Class prefix, CSS properties, whether `auto` is accepted, whether sizes
/// (`full`, fractions) are accepted, whether it takes size keywords (`screen`...).
const FAMILIES: &[(&str, &[&str], bool, bool, bool)] = &[
    ("p", &["padding"], false, false, false),
    ("px", &["padding-left", "padding-right"], false, false, false),
    ("py", &["padding-top", "padding-bottom"], false, false, false),
    ("pt", &["padding-top"], false, false, false),
    ("pr", &["padding-right"], false, false, false),
    ("pb", &["padding-bottom"], false, false, false),
    ("pl", &["padding-left"], false, false, false),
    ("m", &["margin"], true, false, false),
    ("mx", &["margin-left", "margin-right"], true, false, false),
    ("my", &["margin-top", "margin-bottom"], true, false, false),
    ("mt", &["margin-top"], true, false, false),
    ("mr", &["margin-right"], true, false, false),
    ("mb", &["margin-bottom"], true, false, false),
    ("ml", &["margin-left"], true, false, false),
    ("gap", &["gap"], false, false, false),
    ("gap-x", &["column-gap"], false, false, false),
    ("gap-y", &["row-gap"], false, false, false),
    ("w", &["width"], true, true, true),
    ("h", &["height"], true, true, true),
    ("size", &["width", "height"], true, true, false),
    ("top", &["top"], true, true, false),
    ("right", &["right"], true, true, false),
    ("bottom", &["bottom"], true, true, false),
    ("left", &["left"], true, true, false),
    ("inset", &["inset"], true, true, false),
    ("inset-x", &["left", "right"], true, true, false),
    ("inset-y", &["top", "bottom"], true, true, false),
];

/// Whole-class entries: class, declarations.
const FIXED: &[(&str, &str)] = &[
    ("block", "display: block;"), ("inline-block", "display: inline-block;"),
    ("inline", "display: inline;"), ("flex", "display: flex;"),
    ("inline-flex", "display: inline-flex;"), ("grid", "display: grid;"),
    ("inline-grid", "display: inline-grid;"), ("hidden", "display: none;"),
    ("table", "display: table;"), ("contents", "display: contents;"),
    ("static", "position: static;"), ("fixed", "position: fixed;"),
    ("absolute", "position: absolute;"), ("relative", "position: relative;"),
    ("sticky", "position: sticky;"),
    ("flex-row", "flex-direction: row;"), ("flex-col", "flex-direction: column;"),
    ("flex-row-reverse", "flex-direction: row-reverse;"),
    ("flex-col-reverse", "flex-direction: column-reverse;"),
    ("flex-wrap", "flex-wrap: wrap;"), ("flex-nowrap", "flex-wrap: nowrap;"),
    ("flex-1", "flex: 1 1 0%;"), ("flex-auto", "flex: 1 1 auto;"),
    ("flex-initial", "flex: 0 1 auto;"), ("flex-none", "flex: none;"),
    ("grow", "flex-grow: 1;"), ("shrink", "flex-shrink: 1;"),
    ("shrink-0", "flex-shrink: 0;"), ("grow-0", "flex-grow: 0;"),
    ("items-start", "align-items: flex-start;"), ("items-center", "align-items: center;"),
    ("items-end", "align-items: flex-end;"), ("items-stretch", "align-items: stretch;"),
    ("items-baseline", "align-items: baseline;"),
    ("justify-start", "justify-content: flex-start;"),
    ("justify-center", "justify-content: center;"),
    ("justify-end", "justify-content: flex-end;"),
    ("justify-between", "justify-content: space-between;"),
    ("justify-around", "justify-content: space-around;"),
    ("justify-evenly", "justify-content: space-evenly;"),
    ("text-left", "text-align: left;"), ("text-center", "text-align: center;"),
    ("text-right", "text-align: right;"), ("text-justify", "text-align: justify;"),
    ("uppercase", "text-transform: uppercase;"), ("lowercase", "text-transform: lowercase;"),
    ("capitalize", "text-transform: capitalize;"),
    ("underline", "text-decoration-line: underline;"),
    ("line-through", "text-decoration-line: line-through;"),
    ("no-underline", "text-decoration-line: none;"),
    ("italic", "font-style: italic;"),
    ("truncate", "overflow: hidden;\ntext-overflow: ellipsis;\nwhite-space: nowrap;"),
    ("overflow-hidden", "overflow: hidden;"), ("overflow-auto", "overflow: auto;"),
    ("overflow-scroll", "overflow: scroll;"), ("overflow-visible", "overflow: visible;"),
    ("cursor-pointer", "cursor: pointer;"), ("cursor-default", "cursor: default;"),
    ("cursor-not-allowed", "cursor: not-allowed;"),
    ("border", "border-width: 1px;"), ("border-0", "border-width: 0px;"),
    ("border-2", "border-width: 2px;"), ("border-4", "border-width: 4px;"),
    ("border-8", "border-width: 8px;"),
    ("font-thin", "font-weight: 100;"), ("font-extralight", "font-weight: 200;"),
    ("font-light", "font-weight: 300;"), ("font-normal", "font-weight: 400;"),
    ("font-medium", "font-weight: 500;"), ("font-semibold", "font-weight: 600;"),
    ("font-bold", "font-weight: 700;"), ("font-extrabold", "font-weight: 800;"),
    ("font-black", "font-weight: 900;"),
    ("rounded", "border-radius: 0.25rem;"), ("rounded-none", "border-radius: 0px;"),
    ("rounded-sm", "border-radius: 0.125rem;"), ("rounded-md", "border-radius: 0.375rem;"),
    ("rounded-lg", "border-radius: 0.5rem;"), ("rounded-xl", "border-radius: 0.75rem;"),
    ("rounded-2xl", "border-radius: 1rem;"), ("rounded-3xl", "border-radius: 1.5rem;"),
    ("rounded-full", "border-radius: 9999px;"),
    ("shadow-sm", "box-shadow: 0 1px 2px 0 rgb(0 0 0 / 0.05);"),
    ("shadow", "box-shadow: 0 1px 3px 0 rgb(0 0 0 / 0.1), 0 1px 2px -1px rgb(0 0 0 / 0.1);"),
    ("shadow-md", "box-shadow: 0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1);"),
    ("shadow-lg", "box-shadow: 0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1);"),
    ("shadow-xl", "box-shadow: 0 20px 25px -5px rgb(0 0 0 / 0.1), 0 8px 10px -6px rgb(0 0 0 / 0.1);"),
    ("shadow-2xl", "box-shadow: 0 25px 50px -12px rgb(0 0 0 / 0.25);"),
    ("shadow-inner", "box-shadow: inset 0 2px 4px 0 rgb(0 0 0 / 0.05);"),
    ("shadow-none", "box-shadow: none;"),
    ("text-xs", "font-size: 0.75rem;\nline-height: 1rem;"),
    ("text-sm", "font-size: 0.875rem;\nline-height: 1.25rem;"),
    ("text-base", "font-size: 1rem;\nline-height: 1.5rem;"),
    ("text-lg", "font-size: 1.125rem;\nline-height: 1.75rem;"),
    ("text-xl", "font-size: 1.25rem;\nline-height: 1.75rem;"),
    ("text-2xl", "font-size: 1.5rem;\nline-height: 2rem;"),
    ("text-3xl", "font-size: 1.875rem;\nline-height: 2.25rem;"),
    ("text-4xl", "font-size: 2.25rem;\nline-height: 2.5rem;"),
    ("text-5xl", "font-size: 3rem;\nline-height: 1;"),
    ("text-6xl", "font-size: 3.75rem;\nline-height: 1;"),
    ("text-7xl", "font-size: 4.5rem;\nline-height: 1;"),
    ("text-8xl", "font-size: 6rem;\nline-height: 1;"),
    ("text-9xl", "font-size: 8rem;\nline-height: 1;"),
    ("tracking-tighter", "letter-spacing: -0.05em;"), ("tracking-tight", "letter-spacing: -0.025em;"),
    ("tracking-normal", "letter-spacing: 0em;"), ("tracking-wide", "letter-spacing: 0.025em;"),
    ("tracking-wider", "letter-spacing: 0.05em;"), ("tracking-widest", "letter-spacing: 0.1em;"),
    ("leading-none", "line-height: 1;"), ("leading-tight", "line-height: 1.25;"),
    ("leading-snug", "line-height: 1.375;"), ("leading-normal", "line-height: 1.5;"),
    ("leading-relaxed", "line-height: 1.625;"), ("leading-loose", "line-height: 2;"),
    ("order-first", "order: -9999;"), ("order-last", "order: 9999;"), ("order-none", "order: 0;"),
    ("col-span-full", "grid-column: 1 / -1;"), ("row-span-full", "grid-row: 1 / -1;"),
    ("grid-cols-none", "grid-template-columns: none;"), ("grid-rows-none", "grid-template-rows: none;"),
    ("grid-cols-subgrid", "grid-template-columns: subgrid;"), ("grid-rows-subgrid", "grid-template-rows: subgrid;"),
    ("divide-x", "border-right-width: 0px;\nborder-left-width: 1px; /* on > :not([hidden]) ~ :not([hidden]) */"),
    ("divide-y", "border-top-width: 1px;\nborder-bottom-width: 0px; /* on > :not([hidden]) ~ :not([hidden]) */"),
    ("max-w-none", "max-width: none;"), ("max-w-full", "max-width: 100%;"),
    ("max-w-min", "max-width: min-content;"), ("max-w-max", "max-width: max-content;"),
    ("max-w-fit", "max-width: fit-content;"), ("max-w-prose", "max-width: 65ch;"),
    ("min-w-0", "min-width: 0px;"), ("min-w-full", "min-width: 100%;"),
    ("min-w-min", "min-width: min-content;"), ("min-w-max", "min-width: max-content;"),
    ("min-w-fit", "min-width: fit-content;"),
    ("max-h-none", "max-height: none;"), ("max-h-full", "max-height: 100%;"),
    ("max-h-screen", "max-height: 100vh;"), ("max-h-svh", "max-height: 100svh;"),
    ("max-h-lvh", "max-height: 100lvh;"), ("max-h-dvh", "max-height: 100dvh;"),
    ("max-h-min", "max-height: min-content;"), ("max-h-max", "max-height: max-content;"),
    ("max-h-fit", "max-height: fit-content;"),
    ("min-h-0", "min-height: 0px;"), ("min-h-full", "min-height: 100%;"),
    ("min-h-screen", "min-height: 100vh;"), ("min-h-svh", "min-height: 100svh;"),
    ("min-h-lvh", "min-height: 100lvh;"), ("min-h-dvh", "min-height: 100dvh;"),
    ("min-h-min", "min-height: min-content;"), ("min-h-max", "min-height: max-content;"),
    ("min-h-fit", "min-height: fit-content;"),
];

/// `leading-3..10` (config.full.js `lineHeight`).
const LEADING: &[(&str, &str)] = &[
    ("3", "0.75rem"), ("4", "1rem"), ("5", "1.25rem"), ("6", "1.5rem"), ("7", "1.75rem"),
    ("8", "2rem"), ("9", "2.25rem"), ("10", "2.5rem"),
];

/// `max-w-xs..7xl` and the `screen-*` breakpoints (`max-w-screen-md`).
const MAXW: &[(&str, &str)] = &[
    ("xs", "20rem"), ("sm", "24rem"), ("md", "28rem"), ("lg", "32rem"), ("xl", "36rem"),
    ("2xl", "42rem"), ("3xl", "48rem"), ("4xl", "56rem"), ("5xl", "64rem"), ("6xl", "72rem"),
    ("7xl", "80rem"),
];

const SCREENS: &[(&str, &str)] =
    &[("sm", "640px"), ("md", "768px"), ("lg", "1024px"), ("xl", "1280px"), ("2xl", "1536px")];

const STATES: &[(&str, &str)] = &[
    ("hover", ":hover"), ("focus", ":focus"), ("active", ":active"),
    ("focus-visible", ":focus-visible"), ("disabled", ":disabled"), ("first", ":first-child"),
    ("last", ":last-child"), ("odd", ":nth-child(odd)"), ("even", ":nth-child(even)"),
    ("group-hover", ".group:hover &"), ("dark", "dark mode"),
];

/// Utilities renamed in Tailwind v4, v3 name -> v4 name (from the v4 upgrade
/// guide, tailwindcss.com/docs/upgrade-guide). Only classes that still get a
/// hover show the note; the rest sit here for when they are covered.
const V4_RENAMES: &[(&str, &str)] = &[
    ("shadow-sm", "shadow-xs"), ("shadow", "shadow-sm"),
    ("drop-shadow-sm", "drop-shadow-xs"), ("drop-shadow", "drop-shadow-sm"),
    ("blur-sm", "blur-xs"), ("blur", "blur-sm"),
    ("backdrop-blur-sm", "backdrop-blur-xs"), ("backdrop-blur", "backdrop-blur-sm"),
    ("rounded-sm", "rounded-xs"), ("rounded", "rounded-sm"),
    ("outline-none", "outline-hidden"), ("ring", "ring-3"),
];

/// Tailwind v3 default palette, hex without `#`, shades 50, 100..900, 950
/// (from tailwindcss v3.4.17 `src/public/colors.js`).
const SHADES: [&str; 11] = ["50", "100", "200", "300", "400", "500", "600", "700", "800", "900", "950"];
const PALETTE: &[(&str, [&str; 11])] = &[
    ("slate", ["f8fafc", "f1f5f9", "e2e8f0", "cbd5e1", "94a3b8", "64748b", "475569", "334155", "1e293b", "0f172a", "020617"]),
    ("gray", ["f9fafb", "f3f4f6", "e5e7eb", "d1d5db", "9ca3af", "6b7280", "4b5563", "374151", "1f2937", "111827", "030712"]),
    ("zinc", ["fafafa", "f4f4f5", "e4e4e7", "d4d4d8", "a1a1aa", "71717a", "52525b", "3f3f46", "27272a", "18181b", "09090b"]),
    ("neutral", ["fafafa", "f5f5f5", "e5e5e5", "d4d4d4", "a3a3a3", "737373", "525252", "404040", "262626", "171717", "0a0a0a"]),
    ("stone", ["fafaf9", "f5f5f4", "e7e5e4", "d6d3d1", "a8a29e", "78716c", "57534e", "44403c", "292524", "1c1917", "0c0a09"]),
    ("red", ["fef2f2", "fee2e2", "fecaca", "fca5a5", "f87171", "ef4444", "dc2626", "b91c1c", "991b1b", "7f1d1d", "450a0a"]),
    ("orange", ["fff7ed", "ffedd5", "fed7aa", "fdba74", "fb923c", "f97316", "ea580c", "c2410c", "9a3412", "7c2d12", "431407"]),
    ("amber", ["fffbeb", "fef3c7", "fde68a", "fcd34d", "fbbf24", "f59e0b", "d97706", "b45309", "92400e", "78350f", "451a03"]),
    ("yellow", ["fefce8", "fef9c3", "fef08a", "fde047", "facc15", "eab308", "ca8a04", "a16207", "854d0e", "713f12", "422006"]),
    ("lime", ["f7fee7", "ecfccb", "d9f99d", "bef264", "a3e635", "84cc16", "65a30d", "4d7c0f", "3f6212", "365314", "1a2e05"]),
    ("green", ["f0fdf4", "dcfce7", "bbf7d0", "86efac", "4ade80", "22c55e", "16a34a", "15803d", "166534", "14532d", "052e16"]),
    ("emerald", ["ecfdf5", "d1fae5", "a7f3d0", "6ee7b7", "34d399", "10b981", "059669", "047857", "065f46", "064e3b", "022c22"]),
    ("teal", ["f0fdfa", "ccfbf1", "99f6e4", "5eead4", "2dd4bf", "14b8a6", "0d9488", "0f766e", "115e59", "134e4a", "042f2e"]),
    ("cyan", ["ecfeff", "cffafe", "a5f3fc", "67e8f9", "22d3ee", "06b6d4", "0891b2", "0e7490", "155e75", "164e63", "083344"]),
    ("sky", ["f0f9ff", "e0f2fe", "bae6fd", "7dd3fc", "38bdf8", "0ea5e9", "0284c7", "0369a1", "075985", "0c4a6e", "082f49"]),
    ("blue", ["eff6ff", "dbeafe", "bfdbfe", "93c5fd", "60a5fa", "3b82f6", "2563eb", "1d4ed8", "1e40af", "1e3a8a", "172554"]),
    ("indigo", ["eef2ff", "e0e7ff", "c7d2fe", "a5b4fc", "818cf8", "6366f1", "4f46e5", "4338ca", "3730a3", "312e81", "1e1b4b"]),
    ("violet", ["f5f3ff", "ede9fe", "ddd6fe", "c4b5fd", "a78bfa", "8b5cf6", "7c3aed", "6d28d9", "5b21b6", "4c1d95", "2e1065"]),
    ("purple", ["faf5ff", "f3e8ff", "e9d5ff", "d8b4fe", "c084fc", "a855f7", "9333ea", "7e22ce", "6b21a8", "581c87", "3b0764"]),
    ("fuchsia", ["fdf4ff", "fae8ff", "f5d0fe", "f0abfc", "e879f9", "d946ef", "c026d3", "a21caf", "86198f", "701a75", "4a044e"]),
    ("pink", ["fdf2f8", "fce7f3", "fbcfe8", "f9a8d4", "f472b6", "ec4899", "db2777", "be185d", "9d174d", "831843", "500724"]),
    ("rose", ["fff1f2", "ffe4e6", "fecdd3", "fda4af", "fb7185", "f43f5e", "e11d48", "be123c", "9f1239", "881337", "4c0519"]),
];

/// Colour utility prefix and the CSS property it sets; longer prefixes first.
const COLOR_PROPS: &[(&str, &str)] = &[
    ("border-t", "border-top-color"), ("border-r", "border-right-color"),
    ("border-b", "border-bottom-color"), ("border-l", "border-left-color"),
    ("border-x", "border-inline-color"), ("border-y", "border-block-color"),
    ("border", "border-color"), ("bg", "background-color"), ("text", "color"),
    ("fill", "fill"), ("stroke", "stroke"), ("accent", "accent-color"),
    ("caret", "caret-color"), ("decoration", "text-decoration-color"),
    ("outline", "outline-color"),
];

fn hex_rgb(hex: &str) -> Option<[u8; 3]> {
    if hex.len() != 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some([p(0)?, p(2)?, p(4)?])
}

/// `red-500`, `red-500/50`, `black`, `white/25`, `transparent`, `current`.
fn color_declaration(class: &str) -> Option<String> {
    let (prefix, prop) = COLOR_PROPS.iter().find(|(p, _)| {
        class.strip_prefix(p).is_some_and(|r| r.starts_with('-'))
    })?;
    let rest = &class[prefix.len() + 1..];
    let (name, alpha) = match rest.split_once('/') {
        Some((n, a)) => {
            let a: u32 = a.parse().ok()?;
            let ok = matches!(a, 0 | 5 | 10 | 20 | 25 | 30 | 40 | 50 | 60 | 70 | 75 | 80 | 90 | 95 | 100);
            if !ok {
                return None;
            }
            (n, Some(a))
        }
        None => (rest, None),
    };
    let (rgb, label): ([u8; 3], String) = match name {
        "black" => ([0, 0, 0], "#000".into()),
        "white" => ([255, 255, 255], "#fff".into()),
        "transparent" if alpha.is_none() => return Some(format!("{prop}: transparent;")),
        "current" if alpha.is_none() => return Some(format!("{prop}: currentColor;")),
        _ => {
            let (base, shade) = name.rsplit_once('-')?;
            let col = PALETTE.iter().find(|(n, _)| *n == base)?;
            let idx = SHADES.iter().position(|s| *s == shade)?;
            (hex_rgb(col.1[idx])?, format!("#{}", col.1[idx]))
        }
    };
    let [r, g, b] = rgb;
    let value = match alpha {
        Some(a) => format!("rgb({r} {g} {b} / {})", a as f64 / 100.0),
        None => format!("rgb({r} {g} {b})"),
    };
    Some(format!("{prop}: {value}; /* {label} */"))
}

/// `0 | 5 | 10 ... 100` opacity modifier (`bg-red-500/50`).
fn parse_alpha(a: &str) -> Option<u32> {
    let a: u32 = a.parse().ok()?;
    matches!(a, 0 | 5 | 10 | 20 | 25 | 30 | 40 | 50 | 60 | 70 | 75 | 80 | 90 | 95 | 100).then_some(a)
}

/// `#abc` -> `aabbcc`; 6-digit hex passes through. Anything else is rejected.
fn expand_hex(hex: &str) -> Option<String> {
    match hex.len() {
        3 => Some(hex.bytes().flat_map(|b| [b as char; 2]).collect()),
        6 => Some(hex.to_string()),
        _ => None,
    }
}

/// Arbitrary value between the brackets: non-empty, short, with no characters
/// that would break the hover code block. Underscores become spaces, except
/// inside `url(...)` where Tailwind keeps them.
fn sanitize_arbitrary(raw: &str) -> Option<String> {
    if raw.is_empty() || raw.len() > 60 {
        return None;
    }
    if raw.contains(['[', ']', '{', '}', ';', '<', '>', '`']) {
        return None;
    }
    let v = if raw.contains("url(") { raw.to_string() } else { raw.replace('_', " ") };
    (!v.trim().is_empty()).then_some(v)
}

/// A CSS length (`13px`, `0.5rem`, `100%`, `calc(100% - 1rem)`).
fn is_length(v: &str) -> bool {
    let v = v.strip_prefix('-').unwrap_or(v);
    if v == "0" {
        return true;
    }
    for f in ["calc(", "min(", "max(", "clamp("] {
        if v.starts_with(f) && v.ends_with(')') {
            return true;
        }
    }
    let Some(i) = v.find(|c: char| c.is_ascii_alphabetic() || c == '%') else { return false };
    let (num, unit) = v.split_at(i);
    if num.parse::<f64>().is_err() {
        return false;
    }
    matches!(
        unit,
        "px" | "rem" | "em" | "%" | "vw" | "vh" | "svw" | "svh" | "lvw" | "lvh" | "dvw" | "dvh"
            | "vmin" | "vmax" | "ch" | "ex" | "cm" | "mm" | "in" | "pt" | "pc"
    )
}

/// A CSS colour (`#abc`, `rgb(...)`, a named colour like `red`). `var(...)` is
/// deliberately excluded: on `text-`/`border-` it is ambiguous and needs the
/// `length:`/`color:` type hint.
fn is_color(v: &str) -> bool {
    if let Some(hex) = v.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8) && hex.bytes().all(|b| b.is_ascii_hexdigit());
    }
    for f in [
        "rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklab(", "oklch(", "color(",
    ] {
        if v.starts_with(f) && v.ends_with(')') {
            return true;
        }
    }
    !v.is_empty() && v.bytes().all(|b| b.is_ascii_alphabetic())
}

/// Declarations for an arbitrary-value class (`w-[13px]`, `bg-[#1da1f2]/50`,
/// `text-[length:14px]`). The value is shown verbatim; only the property is
/// inferred from the utility prefix.
fn arbitrary(class: &str) -> Option<String> {
    let (neg, body) = match class.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, class),
    };
    let open = body.find('[')?;
    let prefix = body[..open].strip_suffix('-')?;
    let close = body.rfind(']')?;
    if close < open {
        return None;
    }
    let (inner, alpha) = match body[close + 1..].strip_prefix('/') {
        Some(a) => (&body[open + 1..close], Some(parse_alpha(a)?)),
        None if close + 1 == body.len() => (&body[open + 1..close], None),
        None => return None,
    };
    // Type hints (`text-[length:14px]`, `border-[color:red]`).
    let (hint, inner) = match inner.split_once(':') {
        Some((h, rest)) if matches!(h, "length" | "color") => (Some(h), rest),
        _ => (None, inner),
    };
    let value = sanitize_arbitrary(inner)?;

    let with_alpha = |prop: &str, value: &str| -> Option<String> {
        match alpha {
            None => Some(format!("{prop}: {value};")),
            // Tailwind decomposes a hex colour for the opacity modifier.
            Some(a) => {
                let hex = value.strip_prefix('#')?;
                let [r, g, b] = hex_rgb(&expand_hex(hex)?)?;
                Some(format!("{prop}: rgb({r} {g} {b} / {});", a as f64 / 100.0))
            }
        }
    };

    // Utilities where one class name can set different properties: the
    // `length:`/`color:` hint wins, otherwise the value shape decides.
    let ambiguous = match prefix {
        "text" => Some(("font-size", "color")),
        "border" => Some(("border-width", "border-color")),
        _ => None,
    };
    if let Some((len_prop, color_prop)) = ambiguous {
        return match hint {
            Some("length") => (alpha.is_none()).then(|| format!("{len_prop}: {value};")),
            Some("color") => with_alpha(color_prop, &value),
            _ if is_length(&value) => (alpha.is_none()).then(|| format!("{len_prop}: {value};")),
            _ if is_color(&value) => with_alpha(color_prop, &value),
            _ => None,
        };
    }
    // Hints only resolve the text/border ambiguity.
    if hint.is_some() {
        return None;
    }
    // Colour prefixes; `bg-[url(...)]` is a background image.
    if let Some((_, prop)) = COLOR_PROPS.iter().find(|(p, _)| *p == prefix) {
        if *prop == "background-color" && value.starts_with("url(") {
            return (alpha.is_none()).then(|| format!("background-image: {value};"));
        }
        if !(is_color(&value) || value.starts_with("var(")) {
            return None;
        }
        return with_alpha(prop, &value);
    }
    // From here on an opacity modifier makes no sense.
    if alpha.is_some() {
        return None;
    }
    let radius: &[&str] = match prefix {
        "rounded" => &["border-radius"],
        "rounded-t" => &["border-top-left-radius", "border-top-right-radius"],
        "rounded-r" => &["border-top-right-radius", "border-bottom-right-radius"],
        "rounded-b" => &["border-bottom-right-radius", "border-bottom-left-radius"],
        "rounded-l" => &["border-top-left-radius", "border-bottom-left-radius"],
        "rounded-tl" => &["border-top-left-radius"],
        "rounded-tr" => &["border-top-right-radius"],
        "rounded-br" => &["border-bottom-right-radius"],
        "rounded-bl" => &["border-bottom-left-radius"],
        _ => &[],
    };
    if !radius.is_empty() {
        return Some(radius.iter().map(|p| format!("{p}: {value};")).collect::<Vec<_>>().join("\n"));
    }
    let single: Option<&str> = match prefix {
        "z" => Some("z-index"),
        "opacity" => Some("opacity"),
        "shadow" => Some("box-shadow"),
        "leading" => Some("line-height"),
        "tracking" => Some("letter-spacing"),
        "grid-cols" => Some("grid-template-columns"),
        "grid-rows" => Some("grid-template-rows"),
        _ => None,
    };
    if let Some(prop) = single {
        return Some(format!("{prop}: {value};"));
    }
    // Spacing/sizing families take any value verbatim.
    for (fam, props, _, _, _) in FAMILIES {
        if *fam != prefix {
            continue;
        }
        if neg
            && !matches!(
                *fam,
                "m" | "mx" | "my" | "mt" | "mr" | "mb" | "ml" | "top" | "right" | "bottom" | "left"
                    | "inset" | "inset-x" | "inset-y"
            )
        {
            return None;
        }
        let value = if neg && value != "0" { format!("-{value}") } else { value };
        return Some(props.iter().map(|p| format!("{p}: {value};")).collect::<Vec<_>>().join("\n"));
    }
    None
}

fn spacing(key: &str) -> Option<&'static str> {
    SPACING.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

fn percent(num: u32, den: u32) -> String {
    let s = format!("{:.6}", num as f64 * 100.0 / den as f64);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{s}%")
}

/// Value for `key` in a spacing family, with the family's extras.
fn family_value(key: &str, auto: bool, sizes: bool, keywords: bool, neg: bool) -> Option<String> {
    let base: String = if let Some(v) = spacing(key) {
        v.to_string()
    } else if auto && key == "auto" && !neg {
        "auto".into()
    } else if sizes && key == "full" {
        "100%".into()
    } else if sizes && key.contains('/') {
        let (a, b) = key.split_once('/')?;
        let (a, b): (u32, u32) = (a.parse().ok()?, b.parse().ok()?);
        if !(2..=12).contains(&b) || a == 0 || a >= b {
            return None;
        }
        percent(a, b)
    } else if keywords && matches!(key, "screen" | "min" | "max" | "fit") && !neg {
        key.to_string()
    } else {
        return None;
    };
    Some(if neg && base != "0px" { format!("-{base}") } else { base })
}

fn declarations(class: &str) -> Option<String> {
    if class.contains('[') {
        return arbitrary(class);
    }
    if let Some((_, d)) = FIXED.iter().find(|(c, _)| *c == class) {
        return Some(d.to_string());
    }
    // Numeric families: grid columns, z-index, opacity.
    if let Some(n) = class.strip_prefix("grid-cols-").and_then(|n| n.parse::<u32>().ok()) {
        return (1..=12).contains(&n)
            .then(|| format!("grid-template-columns: repeat({n}, minmax(0, 1fr));"));
    }
    if let Some(n) = class.strip_prefix("col-span-").and_then(|n| n.parse::<u32>().ok()) {
        return (1..=12).contains(&n).then(|| format!("grid-column: span {n} / span {n};"));
    }
    if let Some(n) = class.strip_prefix("z-").and_then(|n| n.parse::<u32>().ok()) {
        return matches!(n, 0 | 10 | 20 | 30 | 40 | 50).then(|| format!("z-index: {n};"));
    }
    if let Some(n) = class.strip_prefix("opacity-").and_then(|n| n.parse::<u32>().ok()) {
        let ok = matches!(n, 0 | 5 | 10 | 20 | 25 | 30 | 40 | 50 | 60 | 70 | 75 | 80 | 90 | 95 | 100);
        return ok.then(|| format!("opacity: {};", n as f64 / 100.0));
    }
    if let Some(n) = class.strip_prefix("order-").and_then(|n| n.parse::<u32>().ok()) {
        return (1..=12).contains(&n).then(|| format!("order: {n};"));
    }
    if let Some(n) = class.strip_prefix("grid-rows-").and_then(|n| n.parse::<u32>().ok()) {
        return (1..=12).contains(&n)
            .then(|| format!("grid-template-rows: repeat({n}, minmax(0, 1fr));"));
    }
    if let Some(n) = class.strip_prefix("row-span-").and_then(|n| n.parse::<u32>().ok()) {
        return (1..=12).contains(&n).then(|| format!("grid-row: span {n} / span {n};"));
    }
    if let Some(key) = class.strip_prefix("leading-") {
        if let Some((_, v)) = LEADING.iter().find(|(k, _)| *k == key) {
            return Some(format!("line-height: {v};"));
        }
    }
    if let Some(key) = class.strip_prefix("max-w-") {
        if let Some((_, v)) = MAXW.iter().find(|(k, _)| *k == key) {
            return Some(format!("max-width: {v};"));
        }
        if let Some(px) = key.strip_prefix("screen-") {
            if let Some((_, v)) = SCREENS.iter().find(|(k, _)| *k == px) {
                return Some(format!("max-width: {v};"));
            }
        }
        if let Some(v) = spacing(key) {
            return Some(format!("max-width: {v};"));
        }
    }
    // `max-h-4`, `min-h-8`, `min-w-2`: the spacing scale.
    for (prefix, prop) in [("max-h-", "max-height"), ("min-h-", "min-height"), ("min-w-", "min-width")] {
        if let Some(key) = class.strip_prefix(prefix) {
            if let Some(v) = spacing(key) {
                return Some(format!("{prop}: {v};"));
            }
        }
    }
    // `space-x-4` / `divide-x-2` style sibling utilities (v3 output uses the
    // `> :not([hidden]) ~ :not([hidden])` selector).
    let (neg, body) = match class.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, class),
    };
    if let Some(key) = body.strip_prefix("space-x-").or_else(|| body.strip_prefix("space-y-")) {
        let prop = if body.starts_with("space-x-") { "margin-left" } else { "margin-top" };
        let value = if key == "reverse" {
            "--tw-space-reverse: 1;".to_string()
        } else {
            let mut v = spacing(key)?.to_string();
            if neg && v != "0px" {
                v.insert(0, '-');
            }
            format!("{prop}: {v};")
        };
        return Some(format!("{value} /* on > :not([hidden]) ~ :not([hidden]) */"));
    }
    if let Some(key) = body.strip_prefix("divide-x-").or_else(|| body.strip_prefix("divide-y-")) {
        // Negative divide utilities do not exist; other negatives (-mt-2)
        // fall through to the families below.
        if neg {
            return None;
        }
        let width = match key {
            "0" => "0px",
            "2" => "2px",
            "4" => "4px",
            "8" => "8px",
            _ => return None,
        };
        let (a, b) = if body.starts_with("divide-x-") {
            ("border-right-width", "border-left-width")
        } else {
            ("border-bottom-width", "border-top-width")
        };
        return Some(format!("{a}: 0px;\n{b}: {width}; /* on > :not([hidden]) ~ :not([hidden]) */"));
    }
    if let Some(c) = color_declaration(class) {
        return Some(c);
    }
    let (neg, body) = match class.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, class),
    };
    for (fam, props, auto, sizes, keywords) in FAMILIES {
        let Some(key) = body.strip_prefix(fam).and_then(|r| r.strip_prefix('-')) else { continue };
        if neg && !matches!(*fam, "m" | "mx" | "my" | "mt" | "mr" | "mb" | "ml" | "top" | "right" | "bottom" | "left" | "inset" | "inset-x" | "inset-y") {
            continue;
        }
        let Some(raw) = family_value(key, *auto, *sizes, *keywords, neg) else { continue };
        let value = match raw.as_str() {
            "screen" if *fam == "w" => "100vw".to_string(),
            "screen" => "100vh".to_string(),
            "min" => "min-content".to_string(),
            "max" => "max-content".to_string(),
            "fit" => "fit-content".to_string(),
            _ => raw,
        };
        return Some(props.iter().map(|p| format!("{p}: {value};")).collect::<Vec<_>>().join("\n"));
    }
    None
}

/// Splits `md:hover:-mt-2` into its variants and the utility, ignoring `:`
/// inside `[...]`. A leading `!` (important) is dropped.
fn split_variants(token: &str) -> (Vec<&str>, &str) {
    let mut parts = Vec::new();
    let (mut start, mut depth) = (0, 0i32);
    for (i, c) in token.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            ':' if depth == 0 => {
                parts.push(&token[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    (parts, token[start..].trim_start_matches('!'))
}

fn variant_note(v: &str) -> String {
    if let Some((_, px)) = SCREENS.iter().find(|(n, _)| *n == v) {
        return format!("`@media (min-width: {px})`");
    }
    if let Some((_, s)) = STATES.iter().find(|(n, _)| *n == v) {
        return format!("`{s}`");
    }
    format!("variant `{v}`")
}

/// The class token under byte `col` of `line`, when it sits inside a quoted
/// `class="..."` or `className="..."` value.
fn token_at(line: &str, col: usize) -> Option<&str> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    for key in ["class=", "className="] {
        for (i, _) in line.match_indices(key) {
            if line[..i].chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == ':') {
                continue;
            }
            let rest = &line[i + key.len()..];
            let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else { continue };
            let start = i + key.len() + 1;
            let Some(len) = line[start..].find(q) else { continue };
            let end = start + len;
            if col < start || col > end {
                continue;
            }
            let value = &line[start..end];
            let rel = col - start;
            let ts = value[..rel].rfind(char::is_whitespace).map_or(0, |p| p + value[p..].chars().next().map_or(1, char::len_utf8));
            let te = value[rel..].find(char::is_whitespace).map_or(value.len(), |p| rel + p);
            let tok = &value[ts..te];
            if tok.is_empty() || tok.len() > 100 || tok.contains(['{', '}', '$', '@', '<', '>', '`']) {
                return None;
            }
            return Some(tok);
        }
    }
    None
}

/// Markdown hover for the Tailwind class under byte `col` of `line`.
pub fn help(line: &str, col: usize) -> Option<String> {
    if line.len() > MAX_LINE {
        return None;
    }
    let token = token_at(line, col)?;
    let (variants, utility) = split_variants(token);
    let css = declarations(utility)?;
    let mut md = format!("**Tailwind** `{token}`\n\n```css\n{css}\n```\n");
    if !variants.is_empty() {
        let notes: Vec<String> = variants.iter().map(|v| variant_note(v)).collect();
        md.push_str(&format!("\nApplies with: {}\n", notes.join(", ")));
    }
    if let Some((_, v4)) = V4_RENAMES.iter().find(|(v3, _)| *v3 == utility) {
        md.push_str(&format!("\nTailwind v4: renamed to `{v4}`.\n"));
    }
    md.push_str("\n_Tailwind v3 default theme. `tailwind.config.js` and other v4 differences are not read._");
    Some(md)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(line: &str, needle: &str) -> Option<String> {
        help(line, line.find(needle).unwrap() + 1)
    }

    #[test]
    fn spacing_sizes_and_fixed_classes() {
        let l = "<div class=\"p-4 -mt-2 w-1/2 h-screen text-sm\">";
        assert!(h(l, "p-4").unwrap().contains("padding: 1rem;"));
        assert!(h(l, "-mt-2").unwrap().contains("margin-top: -0.5rem;"));
        assert!(h(l, "w-1/2").unwrap().contains("width: 50%;"));
        assert!(h(l, "h-screen").unwrap().contains("height: 100vh;"));
        let t = h(l, "text-sm").unwrap();
        assert!(t.contains("font-size: 0.875rem;") && t.contains("line-height: 1.25rem;"), "{t}");
        assert!(h("<i class='px-2.5 gap-x-4 mx-auto'>", "px-2.5").unwrap().contains("padding-left: 0.625rem;\npadding-right: 0.625rem;"));
        assert!(h("<i class='mx-auto'>", "mx-auto").unwrap().contains("margin-left: auto;"));
        assert!(h("<i className=\"grid grid-cols-3 z-10 opacity-75\">", "grid-cols-3").unwrap().contains("repeat(3, minmax(0, 1fr))"));
        assert!(h("<i className=\"grid grid-cols-3 z-10 opacity-75\">", "opacity-75").unwrap().contains("opacity: 0.75;"));
        assert!(h("<i class=\"w-1/3\">", "w-1/3").unwrap().contains("width: 33.333333%;"));
        assert!(h("<i class=\"w-2/3\">", "w-2/3").unwrap().contains("width: 66.666667%;"));
    }

    #[test]
    fn palette_colours() {
        let l = "<p class=\"bg-red-500 text-gray-700/50 border-t-blue-600 text-white bg-transparent\">";
        assert!(h(l, "bg-red-500").unwrap().contains("background-color: rgb(239 68 68); /* #ef4444 */"));
        assert!(h(l, "text-gray-700").unwrap().contains("color: rgb(55 65 81 / 0.5);"));
        assert!(h(l, "border-t-blue-600").unwrap().contains("border-top-color: rgb(37 99 235);"));
        assert!(h(l, "text-white").unwrap().contains("color: rgb(255 255 255); /* #fff */"));
        assert!(h(l, "bg-transparent").unwrap().contains("background-color: transparent;"));
        // Sizes still win over colours, and unknown names or shades are ignored.
        assert!(h("<p class=\"text-sm\">", "text-sm").unwrap().contains("font-size"));
        for c in ["bg-red-550", "bg-reddish-500", "bg-red", "text-red-500/33", "bg-cover", "border-collapse"] {
            let l = format!("<p class=\"{c}\">");
            assert_eq!(h(&l, c), None, "{c}");
        }
    }

    #[test]
    fn variants_are_explained() {
        assert_eq!(h("<a class=\"md:hover:bg-red p-2\">", "md:hover"), None);
        let md = h("<a class=\"md:hover:p-2\">", "md:hover").unwrap();
        assert!(md.contains("`@media (min-width: 768px)`") && md.contains("`:hover`"), "{md}");
        assert!(h("<a class=\"!p-4\">", "!p-4").unwrap().contains("padding: 1rem;"));
    }

    #[test]
    fn unknown_dynamic_and_outside_attribute_are_ignored() {
        for (l, n) in [
            ("<a class=\"btn-primary\">", "btn-primary"),
            ("<a class=\"{{ $c }} p-4\">", "{{"),
            ("<a class=\"p-4\">", "<a"),
            ("p-4 text", "p-4"),
            ("<a data-class=\"p-4\">", "p-4"),
            ("<a class=\"w-3/1 w-13/12 p-99 z-15\">", "w-3/1"),
            ("<a class=\"w-3/1 w-13/12 p-99 z-15\">", "p-99"),
            ("<a class=\"w-3/1 w-13/12 p-99 z-15\">", "z-15"),
        ] {
            assert_eq!(h(l, n), None, "{l}");
        }
        assert_eq!(help(&"x".repeat(5000), 10), None);
    }

    #[test]
    fn v4_renames_are_noted() {
        let md = h("<i class=\"shadow-sm\">", "shadow-sm").unwrap();
        assert!(md.contains("box-shadow: 0 1px 2px 0 rgb(0 0 0 / 0.05);"), "{md}");
        assert!(md.contains("Tailwind v4: renamed to `shadow-xs`."), "{md}");
        assert!(h("<i class=\"shadow\">", "shadow").unwrap().contains("renamed to `shadow-sm`"));
        assert!(h("<i class=\"rounded-sm\">", "rounded-sm").unwrap().contains("renamed to `rounded-xs`"));
        assert!(h("<i class=\"rounded\">", "rounded").unwrap().contains("renamed to `rounded-sm`"));
        // Variants do not hide the note, unchanged classes do not get one.
        assert!(h("<i class=\"md:rounded\">", "md:rounded").unwrap().contains("renamed to `rounded-sm`"));
        for c in ["shadow-md", "shadow-lg", "rounded-lg", "rounded-none", "p-4", "text-sm"] {
            let l = format!("<i class=\"{c}\">");
            assert!(!h(&l, c).unwrap().contains("Tailwind v4"), "{c}");
        }
    }

    #[test]
    fn scales_and_sibling_utilities() {
        assert!(h("<i class=\"tracking-tight\">", "tracking").unwrap().contains("letter-spacing: -0.025em;"));
        assert!(h("<i class=\"leading-7\">", "leading-7").unwrap().contains("line-height: 1.75rem;"));
        assert!(h("<i class=\"leading-snug\">", "snug").unwrap().contains("line-height: 1.375;"));
        assert!(h("<i class=\"order-first\">", "order").unwrap().contains("order: -9999;"));
        assert!(h("<i class=\"order-12\">", "order-12").unwrap().contains("order: 12;"));
        assert!(h("<i class=\"grid-rows-3 row-span-2\">", "grid-rows-3").unwrap().contains("grid-template-rows: repeat(3, minmax(0, 1fr));"));
        assert!(h("<i class=\"grid-rows-3 row-span-2\">", "row-span-2").unwrap().contains("grid-row: span 2 / span 2;"));
        assert!(h("<i class=\"col-span-full\">", "col-span").unwrap().contains("grid-column: 1 / -1;"));
        assert!(h("<i class=\"max-w-md\">", "max-w-md").unwrap().contains("max-width: 28rem;"));
        assert!(h("<i class=\"max-w-screen-md\">", "max-w-").unwrap().contains("max-width: 768px;"));
        assert!(h("<i class=\"max-w-4 max-w-prose\">", "max-w-4").unwrap().contains("max-width: 1rem;"));
        assert!(h("<i class=\"max-w-4 max-w-prose\">", "max-w-prose").unwrap().contains("max-width: 65ch;"));
        assert!(h("<i class=\"min-w-min max-h-dvh\">", "min-w-").unwrap().contains("min-width: min-content;"));
        assert!(h("<i class=\"min-w-min max-h-dvh\">", "max-h-").unwrap().contains("max-height: 100dvh;"));
        assert!(h("<i class=\"min-h-8 max-h-96\">", "min-h-8").unwrap().contains("min-height: 2rem;"));
        assert!(h("<i class=\"min-h-8 max-h-96\">", "max-h-96").unwrap().contains("max-height: 24rem;"));
        let sx = h("<i class=\"space-x-4\">", "space-x").unwrap();
        assert!(sx.contains("margin-left: 1rem;") && sx.contains(":not([hidden])"), "{sx}");
        let sy = h("<i class=\"-space-y-2\">", "-space").unwrap();
        assert!(sy.contains("margin-top: -0.5rem;"), "{sy}");
        let dx = h("<i class=\"divide-x-2\">", "divide-x").unwrap();
        assert!(dx.contains("border-left-width: 2px;") && dx.contains("border-right-width: 0px;"), "{dx}");
        let dy = h("<i class=\"divide-y\">", "divide-y").unwrap();
        assert!(dy.contains("border-top-width: 1px;"), "{dy}");
        for c in ["divide-x-3", "space-x-13", "-divide-x-2", "order-13", "grid-rows-13", "leading-11", "max-w-13"] {
            let l = format!("<i class=\"{c}\">");
            assert_eq!(h(&l, c), None, "{c}");
        }
    }

    #[test]
    fn odd_input_does_not_panic() {
        for l in ["class=\"", "class=\"é ü\"", "className='日本 p-4'", "class=\"p-4\u{0301}\"", "class=\"::\"", "class=\"[:]\"", "class=\"-\"", "class=\"!\"", "class=\"w-[日本]\"", "class=\"p-[é] m-[\"", "class=\"bg-[#äää] text-[x\u{0301}]\"", "class=\"w-[]] z-[[\"", ""] {
            for c in 0..=l.len() + 2 {
                let _ = help(l, c);
            }
        }
    }

    #[test]
    fn arbitrary_values() {
        assert!(h("<i class=\"w-[13px]\">", "w-[13px]").unwrap().contains("width: 13px;"));
        assert!(h("<i class=\"-mt-[0.5rem]\">", "-mt-[0.5rem]").unwrap().contains("margin-top: -0.5rem;"));
        assert!(h("<i class=\"px-[calc(100%_-_1rem)]\">", "px-[calc").unwrap().contains("padding-left: calc(100% - 1rem);\npadding-right: calc(100% - 1rem);"));
        assert!(h("<i class=\"size-[3px]\">", "size-").unwrap().contains("width: 3px;\nheight: 3px;"));
        assert!(h("<i class=\"bg-[#1da1f2]\">", "bg-[#1da1f2]").unwrap().contains("background-color: #1da1f2;"));
        assert!(h("<i class=\"bg-[#1da1f2]/50\">", "bg-[#1da1f2]").unwrap().contains("background-color: rgb(29 161 242 / 0.5);"));
        assert!(h("<i class=\"border-t-[#abc]\">", "border-t-").unwrap().contains("border-top-color: #abc;"));
        assert!(h("<i class=\"fill-[rgb(1_2_3)]\">", "fill-").unwrap().contains("fill: rgb(1 2 3);"));
        assert!(h("<i class=\"bg-[var(--brand)]\">", "bg-[var").unwrap().contains("background-color: var(--brand);"));
        assert!(h("<i class=\"text-[14px]\">", "text-[14px]").unwrap().contains("font-size: 14px;"));
        assert!(h("<i class=\"text-[#333]\">", "text-[#333]").unwrap().contains("color: #333;"));
        assert!(h("<i class=\"text-[length:1.2em]\">", "text-[length").unwrap().contains("font-size: 1.2em;"));
        assert!(h("<i class=\"text-[color:var(--c)]\">", "text-[color").unwrap().contains("color: var(--c);"));
        assert!(h("<i class=\"border-[3px]\">", "border-[3px]").unwrap().contains("border-width: 3px;"));
        assert!(h("<i class=\"border-[#abcabc]\">", "border-[#abcabc]").unwrap().contains("border-color: #abcabc;"));
        assert!(h("<i class=\"rounded-tl-[2px]\">", "rounded-tl").unwrap().contains("border-top-left-radius: 2px;"));
        assert!(h("<i class=\"rounded-[0.25em]\">", "rounded-").unwrap().contains("border-radius: 0.25em;"));
        assert!(h("<i class=\"z-[999] opacity-[0.87]\">", "z-[999]").unwrap().contains("z-index: 999;"));
        assert!(h("<i class=\"z-[999] opacity-[0.87]\">", "opacity-").unwrap().contains("opacity: 0.87;"));
        assert!(h("<i class=\"shadow-[0_1px_2px_red]\">", "shadow-").unwrap().contains("box-shadow: 0 1px 2px red;"));
        assert!(h("<i class=\"grid-cols-[1fr_2fr]\">", "grid-cols-").unwrap().contains("grid-template-columns: 1fr 2fr;"));
        assert!(h("<i class=\"grid-rows-[auto_1fr]\">", "grid-rows-").unwrap().contains("grid-template-rows: auto 1fr;"));
        assert!(h("<i class=\"leading-[1.9] tracking-[0.1em]\">", "leading-").unwrap().contains("line-height: 1.9;"));
        assert!(h("<i class=\"leading-[1.9] tracking-[0.1em]\">", "tracking-").unwrap().contains("letter-spacing: 0.1em;"));
        assert!(h("<i class=\"bg-[url('/img/a_b.png')]\">", "bg-[url").unwrap().contains("background-image: url('/img/a_b.png');"));
        let md = h("<i class=\"md:hover:w-[13px]\">", "md:hover").unwrap();
        assert!(md.contains("width: 13px;") && md.contains("`@media (min-width: 768px)`"), "{md}");
    }

    #[test]
    fn arbitrary_rejects_ambiguous_or_dirty() {
        for c in [
            // Empty, unterminated or trailing junk after `]`.
            "w-[]", "w-[13px", "w-[13px]x", "w-[a[b]",
            // `var(...)` on text/border is ambiguous without a type hint.
            "text-[var(--x)]", "border-[var(--x)]",
            // Characters that would break the hover code block.
            "p-[a;b]", "p-[a{b]", "w-[a`b]",
            // Opacity modifiers that do not apply or cannot be decomposed.
            "bg-[#1da1f2]/33", "bg-[#abcd]/50", "z-[1]/50", "p-[3px]/50",
            // Hints only exist for text/border; negative padding is invalid.
            "w-[length:13px]", "-p-[3px]",
            // Unknown arbitrary prefixes stay silent.
            "blur-[4px]", "unknown-[1px]",
        ] {
            let l = format!("<i class=\"{c}\">");
            assert_eq!(h(&l, c), None, "{c}");
        }
        // Over-long arbitrary values are ignored.
        let c = format!("w-[{}]", "1".repeat(61));
        let l = format!("<i class=\"{c}\">");
        assert_eq!(h(&l, &c), None);
    }
}
