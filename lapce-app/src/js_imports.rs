//! Go to the file named in a JS / TS / Vue / Svelte import:
//! `import Card from '@/Components/Card.vue'`, `import x from "~/lib/x"`,
//! `import('./Chart')` and `require('../util')`.
//!
//! `@/` and `~/` are the Laravel Vite / Inertia starter-kit aliases and map to
//! `resources/js` (then `resources/ts`, `resources/vue`). `./` and `../` are
//! relative to the current file. A missing extension is tried as `.vue`,
//! `.ts`, `.tsx`, `.js`, `.jsx`, `.svelte`, `.mjs`, then `index.*` in a folder.
//!
//! Textual only; nothing is executed. Bare package names, URLs and other alias
//! forms are not resolved, `vite.config.js` and `tsconfig.json` are not read,
//! and the result must stay inside the workspace (symlinked files are skipped).
use std::path::{Path, PathBuf};

const ALIAS_ROOTS: &[&str] = &["resources/js", "resources/ts", "resources/vue"];
const EXTS: &[&str] = &["vue", "ts", "tsx", "js", "jsx", "svelte", "mjs"];
const MAX_SPEC: usize = 300;

/// `(specifier, text before its opening quote)` for the string at `col`.
fn spec_at(line: &str, col: usize) -> Option<(&str, &str)> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let i = line[..col].rfind(['\'', '"'])?;
    let q = line[i..].chars().next()?;
    let end = col + line[col..].find(q)?;
    Some((&line[i + 1..end], &line[..i]))
}

fn is_import_context(before: &str) -> bool {
    let t = before.trim_end();
    t.ends_with("from")
        || t.ends_with("import")
        || t.ends_with("import(")
        || t.ends_with("require(")
        || t.ends_with("export * from")
}

fn is_file(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_file())
}

fn with_candidates(base: &Path) -> Option<PathBuf> {
    if is_file(base) {
        return Some(base.to_path_buf());
    }
    let name = base.file_name()?.to_str()?;
    for e in EXTS {
        let p = base.with_file_name(format!("{name}.{e}"));
        if is_file(&p) {
            return Some(p);
        }
    }
    for e in EXTS {
        let p = base.join(format!("index.{e}"));
        if is_file(&p) {
            return Some(p);
        }
    }
    None
}

fn inside(root: &Path, p: &Path) -> bool {
    match (root.canonicalize(), p.canonicalize()) {
        (Ok(r), Ok(p)) => p.starts_with(r),
        _ => false,
    }
}

/// The imported file for the specifier under byte `col`, if it exists.
pub fn resolve(root: &Path, current: &Path, line: &str, col: usize) -> Option<PathBuf> {
    let (spec, before) = spec_at(line, col)?;
    if spec.is_empty() || spec.len() > MAX_SPEC || spec.contains('\0') || !is_import_context(before) {
        return None;
    }
    let spec = spec.split(['?', '#']).next()?;
    let found = if let Some(rest) = spec.strip_prefix("@/").or_else(|| spec.strip_prefix("~/")) {
        if rest.is_empty() || rest.starts_with('/') || rest.split('/').any(|s| s == "..") {
            return None;
        }
        ALIAS_ROOTS.iter().find_map(|r| with_candidates(&root.join(r).join(rest)))?
    } else if spec.starts_with("./") || spec.starts_with("../") {
        with_candidates(&current.parent()?.join(spec))?
    } else {
        return None;
    };
    inside(root, &found).then_some(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(root: &Path, rel: &str) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "x").unwrap();
        p.canonicalize().unwrap()
    }

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    #[test]
    fn alias_and_relative_imports() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let card = touch(r, "resources/js/Components/Card.vue");
        let idx = touch(r, "resources/js/lib/index.ts");
        let util = touch(r, "resources/js/Pages/util.js");
        let page = r.join("resources/js/Pages/Home.vue");
        let go = |l: &str, n: &str| resolve(r, &page, l, at(l, n));
        let l = "import Card from '@/Components/Card.vue'";
        assert_eq!(go(l, "Card.vue").map(|p| p.canonicalize().unwrap()), Some(card.clone()));
        let l = "import Card from \"@/Components/Card\";";
        assert_eq!(go(l, "Components"), Some(card));
        let l = "import { x } from '~/lib'";
        assert_eq!(go(l, "lib"), Some(idx));
        let l = "const m = await import('./util')";
        assert_eq!(go(l, "util"), Some(util.clone()));
        let l = "const u = require('./util.js?v=2')";
        assert_eq!(go(l, "util"), Some(util));
    }

    #[test]
    fn refuses_other_forms() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        touch(r, "resources/js/a.js");
        let page = r.join("resources/js/Home.vue");
        for l in [
            "import x from 'vue'",
            "import x from '@/../../etc/passwd'",
            "import x from '@//a'",
            "import x from '@/'",
            "const s = '@/a'",
            "import x from 'https://x.test/a.js'",
            "import x from '../../../../../../etc/passwd'",
        ] {
            assert_eq!(resolve(r, &page, l, l.len() - 3), None, "{l}");
        }
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        let page = d.path().join("a.vue");
        for l in ["'", "from '", "from 'é", "import '日本'", "", "from '@/é'"] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), &page, l, c);
            }
        }
    }
}
