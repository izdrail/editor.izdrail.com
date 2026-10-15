//! Go to the component file for a tag in a Vue / Svelte / JSX file:
//! `<UserCard`, `<user-card` and `<Admin.Panel` style tags open
//! `resources/js/**/UserCard.vue` (or `.svelte`, `.tsx`, `.jsx`). The kebab-case
//! form is turned into PascalCase. When several files share a name, one under a
//! `Components` folder wins, then the shortest path.
//!
//! Textual only: reads the cursor's line and lists files under
//! `resources/js` (and `resources/ts`, `resources/vue`). Nothing is executed.
//! Bounded: 5000 directory entries, depth 8, `node_modules` and symlinks are
//! skipped. Built-ins such as `<div>`, `<router-link>`, `<Link>` and `<Head>`
//! are not looked up unless a matching file exists.
use std::path::{Path, PathBuf};

const ROOTS: &[&str] = &["resources/js", "resources/ts", "resources/vue"];
const EXTS: &[&str] = &["vue", "svelte", "tsx", "jsx"];
const MAX_ENTRIES: usize = 5000;
const MAX_DEPTH: usize = 8;

fn is_tag_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// The tag name when byte `col` sits inside `<Name`, as written.
fn tag_at(line: &str, col: usize) -> Option<&str> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let start = line[..col]
        .char_indices()
        .rev()
        .find(|(_, c)| !is_tag_char(*c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    if !line[..start].ends_with('<') {
        return None;
    }
    let end = line[start..].find(|c: char| !is_tag_char(c)).map_or(line.len(), |i| start + i);
    let name = &line[start..end];
    (!name.is_empty() && name.len() <= 100 && col <= end).then_some(name)
}

/// `user-card` -> `UserCard`; `UserCard` stays as written.
fn pascal(name: &str) -> String {
    name.split(|c: char| c == '-' || c == '_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut ch = s.chars();
            ch.next().map(|f| f.to_ascii_uppercase().to_string() + ch.as_str()).unwrap_or_default()
        })
        .collect()
}

fn rank(p: &Path) -> (bool, usize) {
    let in_components = p.components().any(|c| c.as_os_str().eq_ignore_ascii_case("components"));
    (!in_components, p.as_os_str().len())
}

/// Component file for the tag under byte `col`, if one exists.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<PathBuf> {
    let name = tag_at(line, col)?;
    let want = pascal(name);
    if want.is_empty() {
        return None;
    }
    let mut best: Option<PathBuf> = None;
    let mut seen = 0usize;
    for r in ROOTS {
        let mut stack = vec![(root.join(r), 0usize)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                seen += 1;
                if seen > MAX_ENTRIES {
                    return best;
                }
                let Ok(ft) = e.file_type() else { continue };
                let p = e.path();
                if ft.is_dir() {
                    if depth < MAX_DEPTH && e.file_name() != "node_modules" {
                        stack.push((p, depth + 1));
                    }
                } else if ft.is_file()
                    && p.file_stem().is_some_and(|s| s == want.as_str())
                    && p.extension().is_some_and(|x| EXTS.iter().any(|k| x == *k))
                    && best.as_ref().is_none_or(|b| rank(&p) < rank(b))
                {
                    best = Some(p);
                }
            }
        }
    }
    best
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
    fn tags_open_component_files() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let card = touch(r, "resources/js/Components/UserCard.vue");
        let _other = touch(r, "resources/js/Pages/Users/UserCard.vue");
        let btn = touch(r, "resources/js/Shared/PrimaryButton.tsx");
        touch(r, "resources/js/node_modules/pkg/Hidden.vue");
        let l = "  <UserCard :user=\"u\" />";
        assert_eq!(resolve(r, l, at(l, "serCard")), Some(card.clone()));
        let l = "<user-card></user-card>";
        assert_eq!(resolve(r, l, 3), Some(card));
        let l = "<PrimaryButton>ok</PrimaryButton>";
        assert_eq!(resolve(r, l, 5), Some(btn));
        let l = "<Hidden />";
        assert_eq!(resolve(r, l, 3), None);
        let l = "<div class=\"x\">";
        assert_eq!(resolve(r, l, 3), None);
        // Not a tag: plain text, or the cursor past the name.
        let l = "UserCard here";
        assert_eq!(resolve(r, l, 3), None);
        let l = "<UserCard :user=\"u\" />";
        assert_eq!(resolve(r, l, at(l, ":user")), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        for l in ["<", "<é", "<日本語 />", "", "<<<>>>", "<a-"] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), l, c);
            }
        }
    }

    #[test]
    fn pascal_cases_kebab() {
        assert_eq!(pascal("user-card"), "UserCard");
        assert_eq!(pascal("UserCard"), "UserCard");
        assert_eq!(pascal("a__b"), "AB");
    }
}
