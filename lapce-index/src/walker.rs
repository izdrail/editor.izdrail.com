//! Filesystem walking and bulk indexing: the "first-open" pass that
//! feeds every workspace file into the project index.

use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use ignore::WalkBuilder;

use crate::{EntryKind, IndexEntry, ProjectIndex, Result};

/// Files larger than this are skipped: they are almost always generated
/// artifacts (minified bundles, maps, dumps) and would bloat the index.
pub const MAX_FILE_BYTES: u64 = 1_048_576;

/// Commit and report progress every this many indexed files.
pub const COMMIT_EVERY: usize = 50;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IndexStats {
    pub files_seen: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub cancelled: bool,
}

/// Language id for a path, from its extension. Deliberately small and
/// local: it covers the editor's target web stack and degrades to the
/// raw extension for everything else. The completion cascade treats a
/// miss as plain text, never as an error.
pub fn language_for_path(path: &Path) -> String {
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase());
    match extension.as_deref() {
        Some("php") | Some("phtml") => "php",
        Some("js") | Some("mjs") | Some("cjs") | Some("jsx") => "javascript",
        Some("ts") | Some("mts") | Some("cts") | Some("tsx") => "typescript",
        Some("vue") => "vue",
        Some("liquid") => "liquid",
        Some("twig") => "twig",
        Some("html") | Some("htm") => "html",
        Some("css") => "css",
        Some("scss") => "scss",
        Some("json") => "json",
        Some("xml") => "xml",
        Some("md") | Some("markdown") => "markdown",
        Some("rs") => "rust",
        Some(other) => other,
        None => "text",
    }
    .to_string()
}

/// Read at most the cap plus one sentinel byte. A file can grow between
/// metadata inspection and reading, so metadata alone is not a memory bound.
fn read_bounded(mut reader: impl Read) -> std::io::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    reader.by_ref().take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        Ok(None)
    } else {
        Ok(Some(bytes))
    }
}

/// Walk `root` (respecting .gitignore and the usual standard filters),
/// index every readable text file, and commit in batches. Returns
/// early with `cancelled` set when `cancel` flips; whatever was
/// committed stays searchable.
pub fn index_workspace(
    root: &Path,
    index: &ProjectIndex,
    cancel: &AtomicBool,
    on_progress: impl FnMut(usize),
) -> Result<IndexStats> {
    let mut stats = IndexStats::default();
    let mut on_progress = on_progress;
    if cancel.load(Ordering::Relaxed) {
        stats.cancelled = true;
        return Ok(stats);
    }
    // Validate before discarding a previously useful cache.
    if !root.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound, "workspace root is not a directory",
        ).into());
    }
    index.clear()?;

    for entry in WalkBuilder::new(root)
            .standard_filters(true)
            .require_git(false)
            .build() {
        if cancel.load(Ordering::Relaxed) {
            stats.cancelled = true;
            break;
        }
        let Ok(entry) = entry else {
            stats.files_skipped += 1;
            continue;
        };
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        stats.files_seen += 1;
        let path = entry.path();

        let Ok(metadata) = path.metadata() else {
            stats.files_skipped += 1;
            continue;
        };
        if metadata.len() > MAX_FILE_BYTES {
            stats.files_skipped += 1;
            continue;
        }

        let Ok(file) = std::fs::File::open(path) else {
            stats.files_skipped += 1;
            continue;
        };
        let Ok(Some(bytes)) = read_bounded(file) else {
            stats.files_skipped += 1;
            continue;
        };
        if bytes.contains(&0) {
            stats.files_skipped += 1;
            continue;
        }
        let body = String::from_utf8_lossy(&bytes);

        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let language = language_for_path(path);
        // Select a grammar explicitly; Vue SFCs are not parsed as JS.
        if language == "javascript" {
            for entry in crate::symbols::javascript_definitions(&relative, &body)? {
                index.add_entry(&entry)?;
            }
        }
        if language == "typescript" {
            let tsx = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("tsx"));
            for entry in crate::symbols::typescript_definitions(&relative, &body, tsx)? {
                index.add_entry(&entry)?;
            }
        }
        index.add_entry(&IndexEntry {
            path: relative,
            kind: EntryKind::File,
            language,
            symbol: None,
            line: 0,
            col: 0,
            body: body.into_owned(),
        })?;
        stats.files_indexed += 1;

        if stats.files_indexed % COMMIT_EVERY == 0 {
            index.commit()?;
            on_progress(stats.files_indexed);
        }
    }

    index.commit()?;
    on_progress(stats.files_indexed);
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lapce-index-walker-{tag}-{unique}"))
    }

    #[test]
    fn bounded_read_accepts_cap_and_rejects_growth_without_reading_everything() {
        let exact = vec![b'a'; MAX_FILE_BYTES as usize];
        assert_eq!(read_bounded(exact.as_slice()).unwrap().unwrap().len(), exact.len());
        // Represents a stream that grew after the earlier metadata check.
        let mut growing = std::io::repeat(b'b');
        assert!(read_bounded(&mut growing).unwrap().is_none());
        assert_eq!(read_bounded(&b""[..]).unwrap(), Some(Vec::new()));
    }

    #[test]
    fn bounded_read_propagates_io_errors() {
        struct Failing;
        impl std::io::Read for Failing {
            fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("fixture failure"))
            }
        }
        assert!(read_bounded(Failing).is_err());
    }

    #[test]
    fn language_mapping_covers_the_web_stack() {
        assert_eq!(language_for_path(Path::new("a/b.php")), "php");
        assert_eq!(language_for_path(Path::new("a/b.js")), "javascript");
        assert_eq!(language_for_path(Path::new("a/b.tsx")), "typescript");
        assert_eq!(language_for_path(Path::new("a/Comp.vue")), "vue");
        assert_eq!(language_for_path(Path::new("a/theme.liquid")), "liquid");
        assert_eq!(language_for_path(Path::new("a/page.twig")), "twig");
        assert_eq!(language_for_path(Path::new("no_extension")), "text");
    }

    #[test]
    fn indexes_text_files_and_respects_gitignore() {
        let dir = temp_dir("walk");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("node_modules/pkg")).unwrap();
        fs::write(dir.join(".gitignore"), "node_modules/\nignored.log\n").unwrap();
        fs::write(
            dir.join("src/checkout.php"),
            "<?php function render_checkout() {}",
        )
        .unwrap();
        fs::write(dir.join("src/app.ts"), "const cartTotal = 1;").unwrap();
        fs::write(dir.join("ignored.log"), "secret ignored content").unwrap();
        fs::write(dir.join("node_modules/pkg/dep.js"), "const dependency = 2;").unwrap();
        fs::write(dir.join("blob.bin"), b"a\0b").unwrap();

        let index = ProjectIndex::in_memory().unwrap();
        let cancel = AtomicBool::new(false);
        let mut reports = Vec::new();
        let stats = index_workspace(&dir, &index, &cancel, |n| reports.push(n)).unwrap();

        assert!(!stats.cancelled);
        assert_eq!(stats.files_indexed, 2);
        assert_eq!(stats.files_seen, 3);
        assert_eq!(stats.files_skipped, 1);
        assert_eq!(reports.last(), Some(&2));

        let php_hits = index.search("render_checkout", 10).unwrap();
        assert_eq!(php_hits.len(), 1);
        assert_eq!(php_hits[0].path, "src/checkout.php");
        assert_eq!(php_hits[0].language, "php");

        let ts_hits = index.search("cartTotal", 10).unwrap();
        assert_eq!(ts_hits.len(), 1);
        assert_eq!(ts_hits[0].language, "typescript");

        assert!(index.search("dependency", 10).unwrap().is_empty());
        assert!(index.search("secret", 10).unwrap().is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn skips_files_over_the_size_cap() {
        let dir = temp_dir("large");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("huge.txt"), vec![b'x'; (MAX_FILE_BYTES + 1) as usize]).unwrap();
        fs::write(dir.join("small.txt"), "tiny but searchable").unwrap();

        let index = ProjectIndex::in_memory().unwrap();
        let cancel = AtomicBool::new(false);
        let stats = index_workspace(&dir, &index, &cancel, |_| {}).unwrap();

        assert_eq!(stats.files_indexed, 1);
        assert_eq!(stats.files_skipped, 1);
        assert_eq!(index.search("tiny", 10).unwrap().len(), 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn repeated_walk_replaces_changed_files_and_removes_deleted_files() {
        let dir = temp_dir("repeat");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("keep.php"), "originaltoken").unwrap();
        fs::write(dir.join("gone.php"), "deletedtoken").unwrap();
        let index = ProjectIndex::in_memory().unwrap();
        let cancel = AtomicBool::new(false);
        index_workspace(&dir, &index, &cancel, |_| {}).unwrap();
        index_workspace(&dir, &index, &cancel, |_| {}).unwrap();
        assert_eq!(index.search("originaltoken", 10).unwrap().len(), 1);
        fs::write(dir.join("keep.php"), "replacementtoken").unwrap();
        fs::remove_file(dir.join("gone.php")).unwrap();
        index_workspace(&dir, &index, &cancel, |_| {}).unwrap();
        assert!(index.search("originaltoken", 10).unwrap().is_empty());
        assert!(index.search("deletedtoken", 10).unwrap().is_empty());
        assert_eq!(index.search("replacementtoken", 10).unwrap().len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn javascript_definitions_feed_prefix_candidates_and_rescan_removes_old_names() {
        let dir = temp_dir("symbols");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("cart.js"), "export function Checkout() {}" ).unwrap();
        let index = ProjectIndex::in_memory().unwrap();
        let cancel = AtomicBool::new(false);
        let stats = index_workspace(&dir, &index, &cancel, |_| {}).unwrap();
        assert_eq!(stats.files_indexed, 1);
        let hits = index.symbol_prefix("check", "javascript", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "cart.js");
        assert_eq!((hits[0].line, hits[0].col), (1, 16));
        fs::write(dir.join("cart.js"), "function Invoice() {}" ).unwrap();
        index_workspace(&dir, &index, &cancel, |_| {}).unwrap();
        assert!(index.symbol_prefix("check", "javascript", 10).unwrap().is_empty());
        assert_eq!(index.symbol_prefix("inv", "javascript", 10).unwrap().len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn typescript_walker_indexes_types_and_tsx_components() {
        let dir = temp_dir("typescript");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("cart.ts"), "interface Cart {}" ).unwrap();
        fs::write(dir.join("page.tsx"), "export function Page() { return <div/>; }" ).unwrap();
        let index = ProjectIndex::in_memory().unwrap();
        index_workspace(&dir, &index, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(index.symbol_prefix("cart", "typescript", 10).unwrap().len(), 1);
        let hits = index.symbol_prefix("page", "typescript", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "page.tsx");
        assert!(index.symbol_prefix("page", "javascript", 10).unwrap().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cancellation_stops_before_indexing() {
        let dir = temp_dir("cancel");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.php"), "echo 'never indexed';").unwrap();

        let index = ProjectIndex::in_memory().unwrap();
        let cancel = AtomicBool::new(true);
        let stats = index_workspace(&dir, &index, &cancel, |_| {}).unwrap();

        assert!(stats.cancelled);
        assert_eq!(stats.files_indexed, 0);
        assert!(index.search("never", 10).unwrap().is_empty());

        fs::remove_dir_all(&dir).ok();
    }
}
