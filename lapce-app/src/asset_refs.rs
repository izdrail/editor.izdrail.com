//! Go to the asset file named in a template or PHP call:
//! - `asset('css/app.css')` (Laravel and Symfony, also `secure_asset`,
//!   `mix('/js/app.js')`, `Vite::asset('resources/img/x.png')`) opens
//!   `public/css/app.css`; Symfony AssetMapper's `assets/` and the older `web/`
//!   folder are tried after `public/`.
//! - `@vite('resources/js/app.js')` and `@vite(['resources/css/app.css', ...])`
//!   open the file relative to the project root.
//!
//! Textual only; nothing is executed. A `?v=1` query or `#hash` is ignored,
//! `..`, absolute paths and URLs are refused, symlinks are not followed and
//! the file must be under the workspace. Vite aliases (`@/...`) are not read.
use std::path::{Path, PathBuf};

const MAX_NAME: usize = 300;
const ASSET_DIRS: &[&str] = &["public", "assets", "web"];
const ASSET_CALLERS: &[&str] = &["asset", "secure_asset", "mix", "Vite::asset", "asset_path"];

/// The quoted string around byte `col`, taking the nearest quote to the left
/// as the opener so `href="{{ asset('x') }}"` yields `x`, not the attribute.
fn string_at(line: &str, col: usize) -> Option<(&str, &str)> {
    let i = line[..col].rfind(['\'', '"'])?;
    let q = line[i..].chars().next()?;
    let start = i + 1;
    let end = col + line[col..].find(q)?;
    Some((&line[start..end], &line[..i]))
}

fn clamp_col(line: &str, col: usize) -> usize {
    let mut c = col.min(line.len());
    while !line.is_char_boundary(c) {
        c -= 1;
    }
    c
}

/// `before` ends with `caller(` for one of `ASSET_CALLERS` (first argument).
fn is_asset_arg(before: &str) -> bool {
    let Some(open) = before.trim_end().strip_suffix('(') else { return false };
    let open = open.trim_end();
    ASSET_CALLERS.iter().any(|c| {
        open.strip_suffix(c).is_some_and(|rest| {
            rest.chars().next_back().is_none_or(|ch| {
                !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '>' | ':'))
            })
        })
    })
}

/// `before` is `@vite(` followed only by `[`, strings and commas.
fn is_vite_arg(before: &str) -> bool {
    let Some(p) = before.rfind("@vite(") else { return false };
    let rest = before[p + 6..].trim();
    let mut r = rest.strip_prefix('[').unwrap_or(rest).trim();
    while !r.is_empty() {
        let Some(q) = r.chars().next().filter(|c| *c == '\'' || *c == '"') else { return false };
        let Some(end) = r[1..].find(q) else { return false };
        r = r[end + 2..].trim_start();
        match r.strip_prefix(',') {
            Some(x) => r = x.trim_start(),
            None => return r.is_empty(),
        }
    }
    true
}

fn clean(s: &str) -> Option<&str> {
    let s = s.split(['?', '#']).next().unwrap_or(s).trim_start_matches('/');
    let ok = !s.is_empty()
        && s.len() <= MAX_NAME
        && !s.contains('\\')
        && !s.contains('\0')
        && !s.contains("://")
        && !s.contains('$')
        && !s.contains('{')
        && s.split('/').all(|p| !p.is_empty() && p != "." && p != "..");
    ok.then_some(s)
}

fn plain_file(root: &Path, rel: &str) -> Option<PathBuf> {
    let p = root.join(rel);
    (p.starts_with(root) && std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_file()))
        .then_some(p)
}

/// The asset file named by the string under byte `col` of `line`.
pub fn resolve(root: &Path, file: &Path, line: &str, col: usize) -> Option<PathBuf> {
    let name = file.file_name()?.to_str()?;
    if !(name.ends_with(".php") || name.ends_with(".twig") || name.ends_with(".html")) {
        return None;
    }
    let col = clamp_col(line, col);
    let (s, before) = string_at(line, col)?;
    let rel = clean(s)?;
    if is_asset_arg(before) {
        return ASSET_DIRS.iter().find_map(|d| plain_file(root, &format!("{d}/{rel}")));
    }
    if is_vite_arg(before) {
        return plain_file(root, rel);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(root: &Path, rel: &str) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "x").unwrap();
        p
    }

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    #[test]
    fn asset_mix_and_vite_calls() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let css = touch(r, "public/css/app.css");
        let js = touch(r, "assets/app.js");
        let vite = touch(r, "resources/js/app.js");
        let vcss = touch(r, "resources/css/app.css");
        let blade = r.join("v.blade.php");
        let twig = r.join("b.twig");
        let l = "<link href=\"{{ asset('css/app.css?v=3') }}\">";
        assert_eq!(resolve(r, &blade, l, at(l, "css/")), Some(css.clone()));
        let l = "<link href=\"{{ mix('/css/app.css') }}\">";
        assert_eq!(resolve(r, &blade, l, at(l, "css/")), Some(css.clone()));
        let l = "<script src=\"{{ asset('app.js') }}\"></script>";
        assert_eq!(resolve(r, &twig, l, at(l, "app.js")), Some(js));
        let l = "@vite(['resources/css/app.css', 'resources/js/app.js'])";
        assert_eq!(resolve(r, &blade, l, at(l, "resources/css")), Some(vcss));
        assert_eq!(resolve(r, &blade, l, at(l, "resources/js")), Some(vite.clone()));
        let l = "@vite('resources/js/app.js')";
        assert_eq!(resolve(r, &blade, l, at(l, "resources")), Some(vite));
        let l = "{{ asset('css/app.css', 'pkg') }}";
        assert_eq!(resolve(r, &twig, l, at(l, "pkg")), None);
    }

    #[test]
    fn refusals() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path().join("site");
        touch(&r, "public/a.css");
        touch(d.path(), "secret.txt");
        let f = r.join("v.twig");
        for l in [
            "{{ asset('../secret.txt') }}",
            "{{ asset('/etc/passwd') }}",
            "{{ asset('https://x/a.css') }}",
            "{{ asset('a/{{x}}.css') }}",
            "{{ myasset('a.css') }}",
            "{{ $x->asset('a.css') }}",
            "{{ other('a.css') }}",
            "@vite(['a.css', foo('b.css')])",
        ] {
            assert_eq!(resolve(&r, &f, l, at(l, "'") + 2), None, "{l}");
        }
        assert_eq!(resolve(&r, &r.join("a.txt"), "asset('a.css')", 8), None);
        let l = "{{ asset('a.css') }}";
        assert!(resolve(&r, &f, l, at(l, "a.css")).is_some());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_not_followed() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path().join("site");
        std::fs::create_dir_all(r.join("public")).unwrap();
        let real = touch(d.path(), "outside.css");
        std::os::unix::fs::symlink(&real, r.join("public/e.css")).unwrap();
        let l = "{{ asset('e.css') }}";
        assert_eq!(resolve(&r, &r.join("v.twig"), l, at(l, "e.css")), None);
    }

    #[test]
    fn hostile_lines_never_panic() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.php");
        let mut x: u64 = 0xabcd_ef01_2345_6789;
        let alpha: Vec<char> = "asset(' \" \\ é日,[]@vite mix/.?#".chars().collect();
        for _ in 0..500 {
            let mut s = String::new();
            for _ in 0..80 {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                s.push(alpha[(x % alpha.len() as u64) as usize]);
            }
            for c in [0, 6, 30, 79, 400] {
                let _ = resolve(d.path(), &f, &s, c);
            }
        }
    }
}
