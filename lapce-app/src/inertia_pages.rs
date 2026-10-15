//! Go to the Inertia page component named in a server-side call:
//! `Inertia::render('Users/Index', ...)`, `inertia('Users/Index')`,
//! `Inertia::render("Users/Index")` and `Route::inertia('/x', 'Users/Index')`
//! open `resources/js/Pages/Users/Index.vue` (or `.jsx`, `.tsx`, `.svelte`,
//! `.js`, `.ts`). `resources/js/pages` (lowercase, the Laravel starter kits)
//! and `resources/js/Pages` are both tried.
//!
//! Textual only: reads the cursor's line and checks which files exist under
//! the workspace root. Nothing is executed. Page names that climb out of the
//! pages folder, or that use a `Module::Page` prefix, are not resolved, and
//! custom `resolve` paths in `app.js` are not read.
use std::path::{Path, PathBuf};

const MAX_NAME: usize = 200;
const DIRS: &[&str] = &["resources/js/Pages", "resources/js/pages"];
const EXTS: &[&str] = &["vue", "tsx", "jsx", "svelte", "ts", "js"];

/// `(string contents, text before the opening quote)` for the quoted string
/// at byte `col`, treating `\'` as part of the string.
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

/// True when `before` ends with a call to one of the Inertia functions, the
/// string being its page argument (for `Route::inertia` the second one).
fn is_page_arg(before: &str) -> bool {
    let t = before.trim_end();
    if let Some(open) = t.strip_suffix('(') {
        let name = open.trim_end();
        let word_start = name
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '\\' || c == '$'))
            .map_or(0, |i| i + 1);
        let callee = &name[word_start..];
        return matches!(callee.trim_start_matches('\\'), "inertia" | "Inertia::render" | "Inertia\\Inertia::render")
            || callee.ends_with("\\Inertia::render");
    }
    // Route::inertia('/path', 'Page')
    if let Some(args) = t.strip_suffix(',') {
        if let Some(p) = args.rfind("Route::inertia(") {
            let inner = &args[p + "Route::inertia(".len()..];
            return !inner.is_empty() && !inner.contains('(');
        }
    }
    false
}

fn safe(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.contains("::")
        && !name.contains('\0')
        && name.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

fn is_plain_file(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_file())
}

/// The page component file for `name`, if one exists.
pub fn page_file(root: &Path, name: &str) -> Option<PathBuf> {
    if !safe(name) {
        return None;
    }
    for d in DIRS {
        for e in EXTS {
            let p = root.join(d).join(format!("{name}.{e}"));
            if is_plain_file(&p) {
                return Some(p);
            }
        }
    }
    None
}

/// Page file for the Inertia call whose page string is under byte `col`.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<PathBuf> {
    let (s, before) = string_at(line, col)?;
    is_page_arg(before).then(|| page_file(root, s)).flatten()
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
    fn render_inertia_and_route_inertia() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let idx = touch(r, "resources/js/Pages/Users/Index.vue");
        let dash = touch(r, "resources/js/pages/dashboard.tsx");
        for l in [
            "return Inertia::render('Users/Index', ['u' => $u]);",
            "return \\Inertia\\Inertia::render(\"Users/Index\");",
            "return inertia('Users/Index');",
            "return inertia( 'Users/Index' );",
        ] {
            assert_eq!(resolve(r, l, at(l, "Users")), Some(idx.clone()), "{l}");
        }
        let l = "Route::inertia('/dash', 'dashboard');";
        assert_eq!(resolve(r, l, at(l, "dashboard")), Some(dash));
        // The route path itself is not a page.
        assert_eq!(resolve(r, l, at(l, "/dash")), None);
    }

    #[test]
    fn other_strings_and_missing_pages_do_not_resolve() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        touch(r, "resources/js/Pages/Users/Index.vue");
        let l = "return view('Users/Index');";
        assert_eq!(resolve(r, l, at(l, "Users")), None);
        let l = "return Inertia::render('Users/Nope');";
        assert_eq!(resolve(r, l, at(l, "Users")), None);
        let l = "return myInertia('Users/Index');";
        assert_eq!(resolve(r, l, at(l, "Users")), None);
        let l = "return Inertia::render('Users/Index', ['a' => 'Users/Index']);";
        assert_eq!(resolve(r, l, l.rfind("Users").unwrap()), None);
    }

    #[test]
    fn traversal_and_odd_names_are_refused() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        touch(r, "resources/js/secret.vue");
        for n in ["../secret", "/etc/passwd", "a//b", "./x", "Mod::Page", "a\\b", ""] {
            assert_eq!(page_file(r, n), None, "{n}");
        }
        let long = "a".repeat(300);
        assert_eq!(page_file(r, &long), None);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_page_is_not_followed() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let real = touch(r, "outside/Evil.vue");
        std::fs::create_dir_all(r.join("resources/js/Pages")).unwrap();
        std::os::unix::fs::symlink(&real, r.join("resources/js/Pages/Evil.vue")).unwrap();
        assert_eq!(page_file(r, "Evil"), None);
    }

    #[test]
    fn hostile_lines_never_panic() {
        let d = tempfile::tempdir().unwrap();
        let mut x: u64 = 0x1234_5678_9abc_def1;
        let alpha: Vec<char> = "Inertia::render(' \" \\ é日,/)Route::inertia".chars().collect();
        for _ in 0..400 {
            let mut s = String::new();
            for _ in 0..70 {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                s.push(alpha[(x % alpha.len() as u64) as usize]);
            }
            for c in [0, 5, 20, 69, 500] {
                let _ = resolve(d.path(), &s, c);
            }
        }
        let l = "Inertia::render('日本/語'";
        assert_eq!(resolve(d.path(), l, 3), None);
    }
}
