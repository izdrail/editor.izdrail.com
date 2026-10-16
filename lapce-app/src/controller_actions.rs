//! Go to a controller method named in a route string: with the cursor on
//! `'show'` in `Route::get('/p/{p}', [PostController::class, 'show'])`, or on
//! the method part of `'PostController@show'` / `'Admin\PostController@show'`,
//! open `public function show(` in `app/Http/Controllers/**/PostController.php`.
//!
//! Also `Route::get('/x', [PostController::class, 'show'])` split over lines:
//! the class must be on the same line as the method string. Textual only;
//! nothing is executed. Inherited methods and invokable controllers
//! (`Route::get('/x', PostController::class)`) are not resolved. Bounded:
//! depth 6, 2000 files, 1 MB per file, symlinks not followed.
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 1_000_000;
const MAX_FILES: usize = 2000;
const MAX_DEPTH: usize = 6;

fn is_id(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `(class, method)` for the method string under byte `col`.
fn target_at(line: &str, col: usize) -> Option<(&str, &str)> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let i = line[..col].rfind(['\'', '"'])?;
    let q = line[i..].chars().next()?;
    let end = col + line[col..].find(q)?;
    let s = &line[i + 1..end];
    if let Some((c, m)) = s.split_once('@') {
        let class = c.rsplit('\\').next()?;
        let ok = |x: &str| !x.is_empty() && x.len() <= 80 && x.chars().all(is_id);
        return (ok(class) && ok(m)).then_some((class, m));
    }
    if s.is_empty() || s.len() > 80 || !s.chars().all(is_id) {
        return None;
    }
    // `Name::class,` right before the string.
    let before = line[..i].trim_end().strip_suffix(',')?.trim_end().strip_suffix("::class")?;
    let cs = before.char_indices().rev().find(|(_, c)| !is_id(*c)).map_or(0, |(p, c)| p + c.len_utf8());
    let class = &before[cs..];
    (!class.is_empty() && class.len() <= 80).then_some((class, s))
}

fn find_class_file(root: &Path, class: &str) -> Option<PathBuf> {
    let want = format!("{class}.php");
    let mut stack = vec![(root.join("app/Http/Controllers"), 0usize)];
    let mut seen = 0usize;
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            seen += 1;
            if seen > MAX_FILES {
                return None;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                if depth < MAX_DEPTH {
                    stack.push((e.path(), depth + 1));
                }
            } else if ft.is_file() && e.file_name() == want.as_str() {
                return Some(e.path());
            }
        }
    }
    None
}

/// Controller file and 0-based line of the method for the string under `col`.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let (class, method) = target_at(line, col)?;
    let p = find_class_file(root, class)?;
    if std::fs::metadata(&p).ok()?.len() > MAX_FILE_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(&p).ok()?;
    let key = format!("function {method}");
    let i = text.lines().position(|l| {
        l.match_indices(&key).any(|(i, _)| {
            l[i + key.len()..].trim_start().starts_with('(')
        })
    })?;
    Some((p, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, text: &str) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    #[test]
    fn route_strings_open_the_controller_method() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let post = put(r, "app/Http/Controllers/Admin/PostController.php", "<?php\nclass PostController {\n    public function index() {}\n    public function show(Post $p) {}\n}");
        let go = |l: &str, n: &str| resolve(r, l, at(l, n));
        let l = "Route::get('/p/{p}', [PostController::class, 'show']);";
        assert_eq!(go(l, "show"), Some((post.clone(), 3)));
        let l = "Route::get('/p', 'Admin\\\\PostController@index');";
        assert_eq!(go(l, "index"), Some((post.clone(), 2)));
        let l = "Route::get('/p', \"PostController@show\");";
        assert_eq!(go(l, "PostController"), Some((post, 3)));
        let l = "Route::get('/p', [PostController::class, 'missing']);";
        assert_eq!(go(l, "missing"), None);
        let l = "Route::get('/p', [Other::class, 'show']);";
        assert_eq!(go(l, "show"), None);
        let l = "foo('show')";
        assert_eq!(go(l, "show"), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "app/Http/Controllers/AController.php", "<?php\nfunction é(");
        for l in ["'@", "'é@é'", "[É::class, 'é']", "::class, '", "'A@", "", "[AController::class,'日本'"] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), l, c);
            }
        }
    }
}
