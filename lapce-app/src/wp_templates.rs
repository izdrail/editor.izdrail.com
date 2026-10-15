//! WordPress theme and plugin file navigation without a language server.
//!
//! Go to Definition on a string opens the file it names:
//! - `get_template_part('template-parts/content', 'page')` opens
//!   `template-parts/content-page.php` (then `content.php`), relative to the
//!   theme folder. `get_header('x')`, `get_footer('x')` and `get_sidebar('x')`
//!   open `header-x.php` (then `header.php`), and so on.
//! - `locate_template('inc/x.php')`.
//! - Paths joined to a base: `get_template_directory() . '/inc/x.php'`,
//!   `get_stylesheet_directory()`, `get_theme_file_path('/inc/x.php')`,
//!   `get_parent_theme_file_path(...)` (theme folder), and `__DIR__ . '/x.php'`,
//!   `plugin_dir_path(__FILE__) . 'x.php'`, `dirname(__FILE__) . '/x.php'` (the
//!   folder of the current file).
//!
//! The theme folder is the nearest folder above the current file that has a
//! `style.css`, else the workspace root. Textual only; nothing is executed.
//! Child-theme overrides are not modelled, `..` and absolute paths are
//! refused, and files must stay under the workspace root (symlinks are not
//! followed).
use std::path::{Path, PathBuf};

const MAX_NAME: usize = 200;

/// `(string contents, text before the opening quote)` for the string at `col`.
fn string_at(line: &str, col: usize) -> Option<(&str, &str)> {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\'' || b[i] == b'"' {
            let q = b[i];
            let start = i + 1;
            let mut j = start;
            while j < b.len() && b[j] != q {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            if j >= b.len() {
                return None;
            }
            if col >= start && col <= j {
                return Some((&line[start..j], &line[..i]));
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    None
}

fn clamp_col(line: &str, col: usize) -> usize {
    let mut c = col.min(line.len());
    while !line.is_char_boundary(c) {
        c -= 1;
    }
    c
}

fn safe_rel(s: &str) -> Option<&str> {
    let s = s.trim_start_matches('/');
    let ok = !s.is_empty()
        && s.len() <= MAX_NAME
        && !s.contains('\\')
        && !s.contains('\0')
        && !s.contains("://")
        && !s.contains('$')
        && s.split('/').all(|p| !p.is_empty() && p != "." && p != "..");
    ok.then_some(s)
}

fn is_plain_file(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_file())
}

fn theme_root(root: &Path, file: &Path) -> PathBuf {
    let mut dir = file.parent();
    while let Some(d) = dir {
        if !d.starts_with(root) {
            break;
        }
        if d.join("style.css").is_file() {
            return d.to_path_buf();
        }
        if d == root {
            break;
        }
        dir = d.parent();
    }
    root.to_path_buf()
}

/// The argument position (0-based) of the string among top-level quoted
/// arguments of the call that encloses it, plus that call's name and the
/// first string argument, e.g. `get_template_part('a', 'b')`.
fn call_context(before: &str) -> Option<(&str, Vec<String>)> {
    let open = before.rfind('(')?;
    let name_end = before[..open].trim_end();
    let start = name_end
        .char_indices()
        .rev()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
        .map_or(0, |(i, c)| i + c.len_utf8());
    let name = &name_end[start..];
    if name.is_empty() || name_end[..start].ends_with("->") || name_end[..start].ends_with("::") {
        return None;
    }
    // Earlier string arguments, `'a', "b", `.
    let mut prev = Vec::new();
    let mut rest = &before[open + 1..];
    loop {
        let t = rest.trim_start();
        let Some(q) = t.chars().next().filter(|c| *c == '\'' || *c == '"') else { break };
        let Some(end) = t[1..].find(q) else { return None };
        prev.push(t[1..1 + end].to_string());
        let after = t[end + 2..].trim_start();
        match after.strip_prefix(',') {
            Some(r) => rest = r,
            None => return None,
        }
    }
    rest.trim().is_empty().then_some((name, prev))
}

/// Base folder when `before` ends with `<base expression> .`.
fn concat_base(before: &str, root: &Path, file: &Path) -> Option<PathBuf> {
    let t = before.trim_end().strip_suffix('.')?.trim_end();
    for f in ["get_template_directory()", "get_stylesheet_directory()"] {
        if t.ends_with(f) {
            return Some(theme_root(root, file));
        }
    }
    let here = || file.parent().map(Path::to_path_buf);
    let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    for f in ["__DIR__", "plugin_dir_path(__FILE__)", "dirname(__FILE__)"] {
        if compact.ends_with(f) {
            return here();
        }
    }
    None
}

fn existing(base: &Path, root: &Path, rel: &str) -> Option<PathBuf> {
    let p = base.join(rel);
    (p.starts_with(root) && is_plain_file(&p)).then_some(p)
}

fn with_php(rel: &str) -> String {
    if rel.ends_with(".php") { rel.to_string() } else { format!("{rel}.php") }
}

/// File named by the WordPress string under byte `col` of `line`.
pub fn resolve(root: &Path, file: &Path, line: &str, col: usize) -> Option<PathBuf> {
    if !file.extension().is_some_and(|e| e == "php") {
        return None;
    }
    let col = clamp_col(line, col);
    let (s, before) = string_at(line, col)?;
    if let Some(base) = concat_base(before, root, file) {
        // `'/a.php' . $x` is only part of a path.
        if line[col..].find(['\'', '"']).is_some_and(|q| line[col + q + 1..].trim_start().starts_with('.')) {
            return None;
        }
        return existing(&base, root, safe_rel(s)?);
    }
    let (name, prev) = call_context(before)?;
    let theme = || theme_root(root, file);
    match (name, prev.len()) {
        ("get_template_part", 0) => {
            let slug = safe_rel(s)?;
            existing(&theme(), root, &with_php(slug))
        }
        ("get_template_part", 1) => {
            let slug = safe_rel(&prev[0])?;
            let name = safe_rel(s)?;
            let t = theme();
            existing(&t, root, &format!("{slug}-{name}.php"))
                .or_else(|| existing(&t, root, &format!("{slug}.php")))
        }
        ("get_header" | "get_footer" | "get_sidebar", 0) => {
            let n = safe_rel(s)?;
            let kind = name.trim_start_matches("get_");
            let t = theme();
            existing(&t, root, &format!("{kind}-{n}.php"))
                .or_else(|| existing(&t, root, &format!("{kind}.php")))
        }
        ("locate_template" | "get_theme_file_path" | "get_parent_theme_file_path", 0) => {
            existing(&theme(), root, safe_rel(s)?)
        }
        _ => None,
    }
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
    fn template_parts_headers_and_locate() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let theme = r.join("wp-content/themes/acme");
        touch(&theme, "style.css");
        let f = touch(&theme, "page.php");
        let cp = touch(&theme, "template-parts/content-page.php");
        let c = touch(&theme, "template-parts/content.php");
        let h = touch(&theme, "header-shop.php");
        let h0 = touch(&theme, "header.php");
        let inc = touch(&theme, "inc/setup.php");

        let l = "get_template_part( 'template-parts/content', 'page' );";
        assert_eq!(resolve(r, &f, l, at(l, "page'")), Some(cp));
        // On the slug the same fallback applies.
        assert_eq!(resolve(r, &f, l, at(l, "template-parts")).map(|p| p.ends_with("content.php")), Some(true));
        let l = "get_template_part('template-parts/content', 'missing');";
        assert_eq!(resolve(r, &f, l, at(l, "missing")), Some(c.clone()));
        let l = "get_template_part('template-parts/content');";
        assert_eq!(resolve(r, &f, l, at(l, "content")), Some(c));
        let l = "get_header('shop');";
        assert_eq!(resolve(r, &f, l, at(l, "shop")), Some(h));
        let l = "get_header('none');";
        assert_eq!(resolve(r, &f, l, at(l, "none")), Some(h0));
        let l = "locate_template('inc/setup.php');";
        assert_eq!(resolve(r, &f, l, at(l, "inc")), Some(inc.clone()));
        let l = "require get_template_directory() . '/inc/setup.php';";
        assert_eq!(resolve(r, &f, l, at(l, "/inc")), Some(inc.clone()));
        let l = "require_once get_theme_file_path('/inc/setup.php');";
        assert_eq!(resolve(r, &f, l, at(l, "/inc")), Some(inc));
    }

    #[test]
    fn plugin_relative_paths() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let main = touch(r, "wp-content/plugins/p/p.php");
        let inc = touch(r, "wp-content/plugins/p/includes/a.php");
        for l in [
            "require_once plugin_dir_path( __FILE__ ) . 'includes/a.php';",
            "require __DIR__ . '/includes/a.php';",
            "include dirname(__FILE__) . '/includes/a.php';",
        ] {
            assert_eq!(resolve(r, &main, l, at(l, "includes")), Some(inc.clone()), "{l}");
        }
    }

    #[test]
    fn refuses_traversal_and_other_calls() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path().join("site");
        let f = touch(&r, "a.php");
        touch(d.path(), "secret.php");
        for l in [
            "require __DIR__ . '/../secret.php';",
            "locate_template('../secret.php');",
            "get_template_part('../secret');",
            "require __DIR__ . '/a.php' . $x;",
            "$o->get_header('a');",
            "my_get_header('a.php');",
            "require __DIR__ . 'http://x/a.php';",
        ] {
            assert_eq!(resolve(&r, &f, l, at(l, "'") + 2), None, "{l}");
        }
        // Non-PHP current file.
        assert_eq!(resolve(&r, &r.join("a.txt"), "get_header('a');", 13), None);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_targets_are_not_followed() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path().join("site");
        let f = touch(&r, "a.php");
        let real = touch(d.path(), "outside.php");
        std::os::unix::fs::symlink(&real, r.join("evil.php")).unwrap();
        let l = "locate_template('evil.php');";
        assert_eq!(resolve(&r, &f, l, at(l, "evil")), None);
    }

    #[test]
    fn hostile_lines_never_panic() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.php");
        let mut x: u64 = 0xdead_beef_cafe_f00d;
        let alpha: Vec<char> = "get_template_part(' \" \\ é日,.)__DIR__ plugin_dir_path(__FILE__)/".chars().collect();
        for _ in 0..500 {
            let mut s = String::new();
            for _ in 0..80 {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                s.push(alpha[(x % alpha.len() as u64) as usize]);
            }
            for c in [0, 7, 30, 79, 400] {
                let _ = resolve(d.path(), &f, &s, c);
            }
        }
    }
}
