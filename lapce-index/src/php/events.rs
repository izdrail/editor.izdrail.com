//! Shared cheap watcher gate. No index writes, parser or full walk.
use super::{cache::blocked, composer::Composer};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
pub fn normalize(path: &Path) -> Option<PathBuf> {
    if let Ok(path) = path.canonicalize() {
        return Some(path);
    }
    let name = path.file_name()?;
    let parent = path.parent()?;
    normalize(parent).map(|p| p.join(name))
}
pub fn relevant(root: &Path, composer: &Composer, path: &Path) -> bool {
    let Some(root) = normalize(root) else {
        return false;
    };
    let root = root.as_path();
    let Some(path) = normalize(path) else {
        return false;
    };
    let path = path.as_path();
    if !path.starts_with(root) || blocked(path) {
        return false;
    }
    // Existing symlinks must never make event filtering read outside the root.
    if let Ok(canonical) = path.canonicalize() {
        if !canonical.starts_with(root) {
            return false;
        }
    }
    let metadata_file = path == root.join("composer.json")
        || path == root.join("composer.lock")
        || path == composer.vendor.join("composer/installed.json")
        || composer.manifests.iter().any(|p| p == path);
    if metadata_file {
        return true;
    }
    let directory = path.is_dir() || (!path.exists() && path.extension().is_none());
    if !directory
        && !matches!(
            path.extension().and_then(|s| s.to_str()),
            Some(
                "php"
                    | "phtml"
                    | "js"
                    | "mjs"
                    | "cjs"
                    | "jsx"
                    | "ts"
                    | "mts"
                    | "cts"
                    | "tsx"
            )
        )
    {
        return false;
    }
    let vendor_autoload = composer
        .roots
        .iter()
        .chain(composer.files.iter())
        .any(|p| p.starts_with(&composer.vendor) && path.starts_with(p));
    if !vendor_autoload {
        let mut ignore = ignore::gitignore::GitignoreBuilder::new(root);
        let mut parents = path.parent();
        let mut files = vec![root.join(".git/info/exclude")];
        while let Some(parent) = parents {
            if !parent.starts_with(root) {
                break;
            }
            files.push(parent.join(".gitignore"));
            files.push(parent.join(".ignore"));
            parents = parent.parent();
        }
        for file in files.into_iter().rev() {
            if file.is_file() {
                ignore.add(file);
            }
        }
        if ignore.build().is_ok_and(|matcher| {
            matcher
                .matched_path_or_any_parents(path, directory)
                .is_ignore()
        }) {
            return false;
        }
    }
    if composer.excluded(path) {
        return false;
    }
    if let Ok(metadata) = path.metadata() {
        if metadata.len() > super::super::walker::MAX_FILE_BYTES && !directory {
            return false;
        }
    }
    if !directory {
        if let Ok(file) = std::fs::File::open(path) {
            let mut bytes = vec![];
            if file.take(8192).read_to_end(&mut bytes).is_err() || bytes.contains(&0)
            {
                return false;
            }
        }
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(unix)]
    fn symlink_workspace_events_and_removed_directories() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("real");
        std::fs::create_dir(&root).unwrap();
        let link = t.path().join("link");
        std::os::unix::fs::symlink(&root, &link).unwrap();
        std::fs::write(root.join("A.php"), "<?php").unwrap();
        let c = Composer::load(&root);
        assert!(relevant(&root, &c, &link.join("A.php")));
        assert!(relevant(&link, &c, &link.join("deleted")));
        assert!(!relevant(&link, &c, &link.join("target/deleted")));
    }
    #[test]
    fn ignored_and_generated_files_do_not_wake() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        std::fs::write(root.join(".gitignore"), "ignored.php\n").unwrap();
        std::fs::write(root.join("ignored.php"), "<?php").unwrap();
        std::fs::write(root.join("binary.php"), [0, 1]).unwrap();
        let c = Composer::load(root);
        for p in [
            "ignored.php",
            "target/generated.php",
            "notes.log",
            "binary.php",
        ] {
            assert!(!relevant(root, &c, &root.join(p)), "{p}");
        }
        assert!(relevant(root, &c, &root.join("A.php")));
        assert!(relevant(root, &c, &root.join("composer.lock")));
    }
}
