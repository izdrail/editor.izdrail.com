//! Go to an Eloquent local scope from its use: `Post::published()` opens
//! `public function scopePublished(` in the `Post` model (also the Laravel 12
//! `#[Scope] protected function published(` form). The receiver must be a
//! static call on a class name, `Post::published()` or `\App\Models\Post::published()`.
//!
//! The model file is looked up as `app/Models/Post.php`, then `app/Post.php`.
//! Textual only; nothing is executed. Chained calls (`->published()`), scopes
//! inherited from traits or parent classes and dynamic `where*` calls are not
//! resolved. Real methods are left to normal Go to Definition.
//! Bounded: 1 MB per file.
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 1_000_000;
const DIRS: &[&str] = &["app/Models", "app"];

pub(crate) fn call_at(line: &str, col: usize) -> Option<(&str, &str)> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let is_id = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let s = line[..col].char_indices().rev().find(|(_, c)| !is_id(*c)).map_or(0, |(i, c)| i + c.len_utf8());
    let e = line[col..].char_indices().find(|(_, c)| !is_id(*c)).map_or(line.len(), |(i, _)| col + i);
    let method = &line[s..e];
    if method.is_empty() || !line[e..].trim_start().starts_with('(') {
        return None;
    }
    let recv = line[..s].strip_suffix("::")?;
    let rs = recv.char_indices().rev().find(|(_, c)| !is_id(*c)).map_or(0, |(i, c)| i + c.len_utf8());
    let class = &recv[rs..];
    let first = class.chars().next()?;
    (first.is_ascii_uppercase() && class.len() <= 80 && method.len() <= 80).then_some((class, method))
}

fn ucfirst(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
}

/// Model file and 0-based line of the scope for the call under `col`.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let (class, method) = call_at(line, col)?;
    let scope_fn = format!("function scope{}(", ucfirst(method));
    let attr_fn = format!("function {method}(");
    for d in DIRS {
        let p = root.join(d).join(format!("{class}.php"));
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let lines: Vec<&str> = text.lines().collect();
        if let Some(i) = lines.iter().position(|l| l.contains(&scope_fn)) {
            return Some((p, i));
        }
        // `#[Scope]` on this line or the one above.
        if let Some(i) = lines.iter().position(|l| l.contains(&attr_fn)) {
            let marked = lines[i].contains("#[Scope]") || (i > 0 && lines[i - 1].trim() == "#[Scope]");
            if marked {
                return Some((p, i));
            }
        }
    }
    None
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
    fn scopes_open_their_method() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let post = put(r, "app/Models/Post.php", "<?php\nclass Post extends Model {\n    public function scopePublished($q) {}\n    #[Scope]\n    protected function recent(Builder $q): void {}\n    public function plain() {}\n}");
        let go = |l: &str, n: &str| resolve(r, l, at(l, n));
        let l = "$p = Post::published()->get();";
        assert_eq!(go(l, "published"), Some((post.clone(), 2)));
        let l = "return \\App\\Models\\Post::recent();";
        assert_eq!(go(l, "recent"), Some((post, 4)));
        let l = "Post::plain()";
        assert_eq!(go(l, "plain"), None);
        let l = "$q->published()";
        assert_eq!(go(l, "published"), None);
        let l = "Comment::published()";
        assert_eq!(go(l, "published"), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "app/Models/Post.php", "<?php\nfunction scopeA(");
        for l in ["::", "Post::", "Post::é(", "É::a(", "日本::日本()", "Post::a(", "", "Post::scopeA(é)"] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), l, c);
            }
        }
    }
}
