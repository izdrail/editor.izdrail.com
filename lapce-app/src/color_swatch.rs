//! Colour swatches: a small filled square before CSS colour literals in
//! stylesheets, templates and scripts: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`,
//! `rgb()`/`rgba()` and `hsl()`/`hsla()` with numeric arguments (comma or
//! space syntax, optional `/ alpha`).
//!
//! Named colours (`red`), `var(--x)`, `currentColor`, Tailwind class names
//! (`bg-red-500`) and other colour functions are not swatched. Per line, only
//! the first 2000 bytes are scanned and at most 12 swatches are produced.
use std::path::Path;

const MAX_LINE: usize = 2000;
const MAX_SWATCHES: usize = 12;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Swatch {
    /// Byte column of the colour literal in the line.
    pub col: usize,
    pub rgba: [u8; 4],
}

/// Whether swatches are drawn for this file.
pub fn enabled_for(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    if name.ends_with(".blade.php") {
        return true;
    }
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref(),
        Some(
            "css" | "scss" | "sass" | "less" | "styl" | "pcss" | "html" | "htm" | "vue"
                | "svelte" | "twig" | "js" | "jsx" | "ts" | "tsx" | "mjs"
        )
    )
}

fn hex_val(c: u8) -> Option<u8> {
    (c as char).to_digit(16).map(|d| d as u8)
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'&' | b'#' | b'$' | b'@')
}

fn parse_hex(b: &[u8], at: usize) -> Option<[u8; 4]> {
    let digits: Vec<u8> = b[at + 1..].iter().take_while(|c| c.is_ascii_alphanumeric()).copied().collect();
    if !matches!(digits.len(), 3 | 4 | 6 | 8) {
        return None;
    }
    let v: Option<Vec<u8>> = digits.iter().map(|c| hex_val(*c)).collect();
    let v = v?;
    Some(match v.len() {
        3 => [v[0] * 17, v[1] * 17, v[2] * 17, 255],
        4 => [v[0] * 17, v[1] * 17, v[2] * 17, v[3] * 17],
        6 => [v[0] * 16 + v[1], v[2] * 16 + v[3], v[4] * 16 + v[5], 255],
        _ => [v[0] * 16 + v[1], v[2] * 16 + v[3], v[4] * 16 + v[5], v[6] * 16 + v[7]],
    })
}

fn number(s: &str) -> Option<(f64, bool)> {
    let s = s.trim();
    let (n, pct) = match s.strip_suffix('%') {
        Some(n) => (n, true),
        None => (s.strip_suffix("deg").unwrap_or(s), false),
    };
    let v: f64 = n.trim().parse().ok()?;
    v.is_finite().then_some((v, pct))
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> [u8; 3] {
    let h = h.rem_euclid(360.0) / 360.0;
    let (s, l) = (s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |t: f64| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    [f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0)]
}

/// Parses `rgb(...)`/`hsl(...)` starting at `at` (the function name).
/// Returns the colour; the literal's end is not needed.
fn parse_fn(line: &str, at: usize) -> Option<[u8; 4]> {
    let rest = &line[at..];
    let (name, args) = rest.split_once('(')?;
    let name = name.to_ascii_lowercase();
    let is_rgb = matches!(name.as_str(), "rgb" | "rgba");
    let is_hsl = matches!(name.as_str(), "hsl" | "hsla");
    if !is_rgb && !is_hsl {
        return None;
    }
    let args = &args[..args.find(')')?];
    let parts: Vec<&str> = args
        .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 3 && parts.len() != 4 {
        return None;
    }
    let nums: Option<Vec<(f64, bool)>> = parts.iter().map(|p| number(p)).collect();
    let nums = nums?;
    let alpha = match nums.get(3) {
        Some((a, true)) => a / 100.0,
        Some((a, false)) => *a,
        None => 1.0,
    }
    .clamp(0.0, 1.0);
    let a = (alpha * 255.0).round() as u8;
    if is_rgb {
        let ch = |(v, pct): (f64, bool)| {
            (if pct { v * 255.0 / 100.0 } else { v }).round().clamp(0.0, 255.0) as u8
        };
        Some([ch(nums[0]), ch(nums[1]), ch(nums[2]), a])
    } else {
        let [r, g, b] = hsl_to_rgb(nums[0].0, nums[1].0 / 100.0, nums[2].0 / 100.0);
        Some([r, g, b, a])
    }
}

/// Swatches for one line, left to right.
pub fn find(line: &str) -> Vec<Swatch> {
    let mut end = line.len().min(MAX_LINE);
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    let line = &line[..end];
    if !line.contains('#') && !line.contains("(") {
        return vec![];
    }
    let b = line.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() && out.len() < MAX_SWATCHES {
        match b[i] {
            b'#' if i == 0 || !is_word(b[i - 1]) => {
                if let Some(rgba) = parse_hex(b, i) {
                    out.push(Swatch { col: i, rgba });
                }
                i += 1;
            }
            b'r' | b'R' | b'h' | b'H' if (i == 0 || !is_word(b[i - 1])) => {
                if let Some(rgba) = parse_fn(line, i) {
                    out.push(Swatch { col: i, rgba });
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(line: &str) -> Option<[u8; 4]> {
        find(line).first().map(|s| s.rgba)
    }

    #[test]
    fn hex_forms() {
        assert_eq!(one("color: #fff;"), Some([255, 255, 255, 255]));
        assert_eq!(one("color: #1a2B3c;"), Some([0x1a, 0x2b, 0x3c, 255]));
        assert_eq!(one("c: #f008"), Some([255, 0, 0, 0x88]));
        assert_eq!(one("c: #11223344;"), Some([0x11, 0x22, 0x33, 0x44]));
        assert_eq!(find("a { c: #abc; d: #def }").len(), 2);
        assert_eq!(find("a { c: #abc; d: #def }")[0].col, 7);
    }

    #[test]
    fn hex_false_positives_are_skipped() {
        assert_eq!(one("#12345"), None);
        assert_eq!(one("see issue #12 and #1234567"), None);
        assert_eq!(one("&#8364; &#xff;"), None);
        assert_eq!(one("a#fff"), None);
        assert_eq!(one("#ggg"), None);
        assert_eq!(one("color: #ffffff1"), None);
        assert_eq!(one("$#abc"), None);
    }

    #[test]
    fn rgb_and_hsl() {
        assert_eq!(one("rgb(255, 0, 128)"), Some([255, 0, 128, 255]));
        assert_eq!(one("rgba(0,0,0,.5)"), Some([0, 0, 0, 128]));
        assert_eq!(one("rgb(0 128 255 / 50%)"), Some([0, 128, 255, 128]));
        assert_eq!(one("rgb(100%, 0%, 50%)"), Some([255, 0, 128, 255]));
        assert_eq!(one("hsl(0, 100%, 50%)"), Some([255, 0, 0, 255]));
        assert_eq!(one("hsl(120deg 100% 25%)"), Some([0, 128, 0, 255]));
        assert_eq!(one("hsla(240, 100%, 50%, 0.5)"), Some([0, 0, 255, 128]));
        assert_eq!(one("RGB(1,2,3)"), Some([1, 2, 3, 255]));
        assert_eq!(one("rgb(300, -5, 0)"), Some([255, 0, 0, 255]));
    }

    #[test]
    fn non_literals_are_skipped() {
        assert_eq!(one("color: red;"), None);
        assert_eq!(one("rgb(var(--r), 0, 0)"), None);
        assert_eq!(one("rgb(1, 2)"), None);
        assert_eq!(one("rgb(1, 2, 3, 4, 5)"), None);
        assert_eq!(one("rgb(1, 2, 3"), None);
        assert_eq!(one("brgb(1,2,3)"), None);
        assert_eq!(one("bg-red-500"), None);
        assert_eq!(one("hsl(a, b, c)"), None);
        assert_eq!(one("rgb(NaN, inf, 1)"), None);
        assert_eq!(one(""), None);
    }

    #[test]
    fn limits_and_odd_input() {
        let many = "#fff ".repeat(100);
        assert_eq!(find(&many).len(), MAX_SWATCHES);
        let long = format!("{}#fff", "é".repeat(5000));
        assert!(find(&long).is_empty());
        let _ = find("rgb(");
        let _ = find("#");
        let _ = find("é#fffé rgb(é)");
        let _ = find(&"rgb(".repeat(2000));
    }

    #[test]
    fn enabled_paths() {
        assert!(enabled_for(Path::new("a.css")));
        assert!(enabled_for(Path::new("resources/views/a.blade.php")));
        assert!(enabled_for(Path::new("a.html.twig")));
        assert!(enabled_for(Path::new("A.TSX")));
        assert!(!enabled_for(Path::new("a.md")));
        assert!(!enabled_for(Path::new("a.php")));
        assert!(!enabled_for(Path::new("a.rs")));
    }
}
