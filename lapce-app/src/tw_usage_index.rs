//! "CSS / Tailwind Classes in Use" palette: every class name written in a
//! `class="..."` (or `className="..."`) attribute under `resources/`, with how
//! often it appears and where it first appears. Pick one to jump to its first
//! use. Useful for spotting near-duplicates (`p-4` vs `p-[16px]`) and for
//! finding where a utility is used before changing it.
//!
//! The list comes only from your own files: nothing is guessed about Tailwind
//! itself, so it works for Bootstrap or hand-written CSS too. Static text
//! scan, nothing is executed. Bounded: depth 8, 3000 files, 1 MB per file,
//! 2000 bytes per attribute, 20000 distinct classes kept, 1000 listed,
//! symlinks skipped.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::route_index::RouteTarget;

const MAX_DEPTH: usize = 8;
const MAX_FILES: usize = 3000;
const MAX_FILE: u64 = 1 << 20;
const MAX_ATTR: usize = 2000;
const MAX_DISTINCT: usize = 20_000;
const MAX_LISTED: usize = 1000;
const MAX_TOKEN: usize = 80;
const EXTS: &[&str] = &["php", "html", "vue", "svelte", "jsx", "tsx", "astro", "twig"];

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_FILES {
        return;
    }
    if depth == 0 && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink()) {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if out.len() >= MAX_FILES {
            return;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() {
            walk(&p, depth + 1, out);
        } else if ft.is_file() && p.extension().and_then(|x| x.to_str()).is_some_and(|x| EXTS.contains(&x)) {
            out.push(p);
        }
    }
}

fn plain_class(tok: &str) -> bool {
    !tok.is_empty()
        && tok.len() <= MAX_TOKEN
        && tok.chars().all(|c| c.is_ascii_alphanumeric() || "-_:/.[]%#!,".contains(c))
        && tok.chars().next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '!' || c == '[')
}

/// `(class, 0-based line)` for every plain class token in `class="..."` /
/// `className="..."` attributes of `text`. Tokens with template syntax
/// (`{{ }}`, `<?php`, `$var`, `@if`) are skipped.
pub fn classes_in(text: &str) -> Vec<(String, usize)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    let mut line = 0;
    let mut line_pos = 0;
    while from < text.len() {
        let Some(rel) = text[from..].find("lass") else { break };
        let at = from + rel; // index of "lass"
        from = at + 4;
        // `class=` or `className=`, not `subclass=` or `xclass=`.
        if at == 0 || bytes[at - 1] != b'c' || (at >= 2 && bytes[at - 2].is_ascii_alphanumeric()) {
            continue;
        }
        let mut rest = &text[at + 4..];
        if let Some(r) = rest.strip_prefix("Name") {
            rest = r;
        }
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else { continue };
        let body_start = text.len() - rest.len() + 1;
        let mut limit = (body_start + MAX_ATTR).min(text.len());
        while !text.is_char_boundary(limit) {
            limit -= 1;
        }
        let Some(end) = text[body_start..limit].find(q) else { continue };
        let body = &text[body_start..body_start + end];
        line += text[line_pos..at].matches('\n').count();
        line_pos = at;
        let mut off = 0;
        for tok in body.split_whitespace() {
            let tpos = body[off..].find(tok).map_or(off, |p| off + p);
            let extra = body[..tpos].matches('\n').count();
            off = tpos + tok.len();
            if plain_class(tok) {
                out.push((tok.to_string(), line + extra));
            }
        }
        from = body_start + end + 1;
    }
    out
}

pub fn discover(root: &Path) -> Vec<RouteTarget> {
    let mut files = Vec::new();
    walk(&root.join("resources"), 0, &mut files);
    // class -> (count, first file index, first line)
    let mut seen: HashMap<String, (usize, usize, usize)> = HashMap::new();
    for (fi, f) in files.iter().enumerate() {
        if std::fs::symlink_metadata(f).map_or(true, |m| !m.is_file() || m.len() > MAX_FILE) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        for (c, line) in classes_in(&text) {
            if let Some(e) = seen.get_mut(&c) {
                e.0 += 1;
            } else if seen.len() < MAX_DISTINCT {
                seen.insert(c, (1, fi, line));
            }
        }
    }
    let mut v: Vec<_> = seen.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then_with(|| a.0.cmp(&b.0)));
    v.truncate(MAX_LISTED);
    v.into_iter()
        .map(|(class, (n, fi, line))| {
            let path = files[fi].clone();
            let shown = path.strip_prefix(root).unwrap_or(&path).display().to_string();
            RouteTarget { label: format!("{class}  x{n}  {shown}:{}", line + 1), path, line }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn extracts_plain_classes_with_lines() {
        let t = "<div class=\"p-4 md:flex  hover:bg-red-500/50\">\n<span className='w-[10px] !mt-2'></span>\n<p\n class=\"a\n   b {{ $x }} <?php ?> c\"></p>\n<i subclass=\"no\" xclass=\"no\"></i>\n";
        let got: Vec<_> = classes_in(t).into_iter().map(|(c, l)| format!("{c}@{l}")).collect();
        assert_eq!(
            got,
            ["p-4@0", "md:flex@0", "hover:bg-red-500/50@0", "w-[10px]@1", "!mt-2@1", "a@3", "b@4", "c@4"]
        );
    }

    #[test]
    fn junk_never_panics_or_runs_away() {
        assert!(classes_in("").is_empty());
        assert!(classes_in("class=\"unterminated").is_empty());
        assert!(classes_in("class=").is_empty());
        assert!(classes_in("class=é").is_empty());
        assert_eq!(classes_in("class=\"é日 a\"").len(), 1);
        let long = format!("class=\"{}\"", "x".repeat(100_000));
        assert!(classes_in(&long).is_empty());
        let many = "class=\"a\" ".repeat(50_000);
        assert_eq!(classes_in(&many).len(), 50_000);
    }

    #[test]
    fn discover_counts_orders_and_jumps_to_first_use() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "resources/views/a.blade.php", "<div class=\"p-4 flex\"></div>\n<b class=\"p-4\"></b>\n");
        put(r, "resources/js/B.vue", "<template><i class=\"flex p-4 mt-1\"/></template>");
        put(r, "resources/views/notes.txt", "class=\"ignored\"");
        let got = discover(r);
        let labels: Vec<_> = got.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels.len(), 3);
        assert!(labels[0].starts_with("p-4  x3  resources/js/B.vue:1"), "{labels:?}");
        assert!(labels[1].starts_with("flex  x2"));
        assert!(labels[2].starts_with("mt-1  x1  resources/js/B.vue:1"));
        assert_eq!(got[0].line, 0);
        // First use is in path order: resources/js comes before resources/views.
        assert!(got[0].path.ends_with("B.vue"));
        assert!(discover(&r.join("none")).is_empty());
    }

    #[test]
    fn caps_and_symlinks() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let body: String = (0..3000).map(|i| format!("<i class=\"c{i}\"></i>\n")).collect();
        put(r, "resources/views/many.blade.php", &body);
        put(r, "resources/views/huge.blade.php", &format!("<i class=\"x\"></i>{}", "y".repeat(2_000_000)));
        assert_eq!(discover(r).len(), MAX_LISTED);
        #[cfg(unix)]
        {
            let d2 = tempfile::tempdir().unwrap();
            put(&d2.path().join("real"), "a.blade.php", "<i class=\"a\"></i>");
            std::os::unix::fs::symlink(d2.path().join("real"), d2.path().join("resources")).unwrap();
            assert!(discover(d2.path()).is_empty());
        }
    }

    #[test]
    fn random_text_never_panics() {
        let pieces = ["class=\"", "className='", "\"", "'", " ", "\n", "é", "日", "p-4", "{{", "}}", "class", "lass", "=", "Name", "<?php"];
        let mut s = 21u64;
        for _ in 0..1000 {
            let mut t = String::new();
            for _ in 0..80 {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                t.push_str(pieces[(s >> 33) as usize % pieces.len()]);
            }
            let _ = classes_in(&t);
        }
    }
}
