//! Hover help for Tailwind utility classes in `class="..."` / `className="..."`
//! attributes: hovering `md:hover:p-4` shows `padding: 1rem;` and the variants
//! that apply (`@media (min-width: 768px)`, `:hover`).
//!
//! Covers the Tailwind v3 default theme only (values checked against
//! tailwindcss v3.4.17 `stubs/config.full.js`): spacing utilities (padding,
//! margin, gap, width, height, size, inset), rounded, shadow, text sizes, font
//! weights, display, position, flex, alignment, grid columns, z-index, opacity
//! and a few more. Colours, arbitrary values (`w-[13px]`), custom theme values
//! from `tailwind.config.js` and Tailwind v4 differences are not covered; a
//! class that is not in the table gets no hover. Nothing is executed and no file
//! is read: only the hovered line is looked at.

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
];

const SCREENS: &[(&str, &str)] =
    &[("sm", "640px"), ("md", "768px"), ("lg", "1024px"), ("xl", "1280px"), ("2xl", "1536px")];

const STATES: &[(&str, &str)] = &[
    ("hover", ":hover"), ("focus", ":focus"), ("active", ":active"),
    ("focus-visible", ":focus-visible"), ("disabled", ":disabled"), ("first", ":first-child"),
    ("last", ":last-child"), ("odd", ":nth-child(odd)"), ("even", ":nth-child(even)"),
    ("group-hover", ".group:hover &"), ("dark", "dark mode"),
];

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
    if utility.contains('[') {
        return None;
    }
    let css = declarations(utility)?;
    let mut md = format!("**Tailwind** `{token}`\n\n```css\n{css}\n```\n");
    if !variants.is_empty() {
        let notes: Vec<String> = variants.iter().map(|v| variant_note(v)).collect();
        md.push_str(&format!("\nApplies with: {}\n", notes.join(", ")));
    }
    md.push_str("\n_Tailwind v3 default theme. `tailwind.config.js` and v4 differences are not read._");
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
            ("<a class=\"w-[13px]\">", "w-[13px]"),
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
    fn odd_input_does_not_panic() {
        for l in ["class=\"", "class=\"é ü\"", "className='日本 p-4'", "class=\"p-4\u{0301}\"", "class=\"::\"", "class=\"[:]\"", "class=\"-\"", "class=\"!\"", ""] {
            for c in 0..=l.len() + 2 {
                let _ = help(l, c);
            }
        }
    }
}
