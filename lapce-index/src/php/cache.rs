//! Incremental reconciliation over the existing Tantivy writer. No second store.
use super::{
    composer::Composer,
    scanner::{self, FileSymbols, Symbol},
};
use crate::{EntryKind, IndexEntry, ProjectIndex, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
const VERSION: u32 = 3;
const MAX_BYTES: u64 = crate::walker::MAX_FILE_BYTES;
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CachedFile {
    size: u64,
    modified: u128,
    hash: String,
    symbols: FileSymbols,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Cache {
    version: u32,
    config: String,
    files: BTreeMap<PathBuf, CachedFile>,
}
#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub visited: usize,
    pub parsed: usize,
    pub reused: usize,
    pub removed: usize,
    pub cancelled: bool,
    pub warnings: Vec<String>,
}
pub struct Engine {
    pub index: ProjectIndex,
    cache: Cache,
    cache_path: PathBuf,
    hot: Mutex<BTreeMap<PathBuf, FileSymbols>>,
}
fn metadata(path: &Path) -> std::io::Result<(u64, u128)> {
    let m = path.metadata()?;
    Ok((
        m.len(),
        m.modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ))
}
fn key(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
pub fn blocked(path: &Path) -> bool {
    path.components().any(|p| {
        matches!(
            p.as_os_str().to_str(),
            Some(".git" | "node_modules" | "target" | ".idea" | ".cache")
        )
    })
}
fn collect(
    root: &Path,
    composer: &Composer,
    cancel: &AtomicBool,
) -> BTreeSet<PathBuf> {
    let paths = Mutex::new(BTreeSet::new());
    let mut roots = vec![(root.to_owned(), false)];
    roots.extend(
        composer
            .roots
            .iter()
            .chain(composer.files.iter())
            .map(|p| (p.clone(), p.starts_with(&composer.vendor))),
    );
    for (base, explicit) in roots {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let base = base.canonicalize().unwrap_or(base);
        let mut builder = ignore::WalkBuilder::new(&base);
        builder.require_git(false).follow_links(false).threads(2);
        if explicit {
            builder
                .git_ignore(false)
                .git_exclude(false)
                .git_global(false);
        }
        builder.filter_entry(|e| !blocked(e.path()));
        builder.build_parallel().run(|| {
            Box::new(|entry| {
                if cancel.load(Ordering::Relaxed) {
                    return ignore::WalkState::Quit;
                }
                if let Ok(entry) = entry {
                    if entry.file_type().is_some_and(|f| f.is_file())
                        && !composer.excluded(entry.path())
                    {
                        if let Ok(path) = entry.path().canonicalize() {
                            if path.starts_with(root) {
                                paths.lock().unwrap().insert(path);
                            }
                        }
                    }
                }
                ignore::WalkState::Continue
            })
        });
    }
    paths.into_inner().unwrap()
}
impl Engine {
    pub fn open(dir: &Path) -> Result<Self> {
        let index = ProjectIndex::open_or_create(dir)?;
        let cache_path = dir.join("php-symbol-cache-v1.json");
        // Bounded cache read. Invalid/version-old cache triggers an atomic rebuild,
        // never deletes the store to work around schema/lock/corruption failures.
        let cache = fs::File::open(&cache_path)
            .ok()
            .and_then(|f| {
                let mut bytes = Vec::new();
                f.take(64 * 1024 * 1024 + 1).read_to_end(&mut bytes).ok()?;
                if bytes.len() > 64 * 1024 * 1024 {
                    None
                } else {
                    serde_json::from_slice::<Cache>(&bytes).ok()
                }
            })
            .filter(|c| {
                c.version == VERSION
                    && !dir.join("php-cache-commit.pending").exists()
            })
            .unwrap_or_default();
        let hot = Mutex::new(
            cache
                .files
                .iter()
                .map(|(p, f)| (p.clone(), f.symbols.clone()))
                .collect(),
        );
        Ok(Self {
            index,
            cache,
            cache_path,
            hot,
        })
    }
    pub fn symbols(&self, prefix: &str, limit: usize) -> Vec<(PathBuf, Symbol)> {
        let prefix = prefix.to_lowercase();
        self.hot
            .lock()
            .unwrap()
            .iter()
            .flat_map(|(p, f)| {
                f.symbols
                    .iter()
                    .filter(|s| {
                        s.name.to_lowercase().starts_with(&prefix)
                            || s.qualified.to_lowercase().starts_with(&prefix)
                    })
                    .map(move |s| (p.clone(), s.clone()))
            })
            .take(limit.min(1000))
            .collect()
    }
    pub fn snapshot(&self) -> BTreeMap<PathBuf, FileSymbols> {
        self.hot.lock().unwrap().clone()
    }
    pub fn reconcile(
        &mut self,
        root: &Path,
        dirty: &BTreeSet<PathBuf>,
        cancel: &AtomicBool,
    ) -> Result<Progress> {
        let canonical = root.canonicalize()?;
        let root = canonical.as_path();
        let dirty = dirty
            .iter()
            .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
            .collect::<BTreeSet<_>>();
        let composer = Composer::load(root);
        let mut progress = Progress {
            warnings: composer.warnings.clone(),
            ..Default::default()
        };
        let candidates = collect(root, &composer, cancel);
        let force = self.cache.version != VERSION
            || self.cache.config != composer.fingerprint;
        let mut next = Cache {
            version: VERSION,
            config: composer.fingerprint.clone(),
            files: BTreeMap::new(),
        };
        let mut replacements = Vec::new();
        for path in candidates {
            if cancel.load(Ordering::Relaxed) {
                progress.cancelled = true;
                return Ok(progress);
            }
            progress.visited += 1;
            let Ok((size, modified)) = metadata(&path) else {
                continue;
            };
            if size > MAX_BYTES {
                continue;
            }
            let old = self.cache.files.get(&path);
            if !force
                && !dirty.iter().any(|changed| path.starts_with(changed))
                && old.is_some_and(|f| f.size == size && f.modified == modified)
            {
                next.files.insert(path, old.unwrap().clone());
                progress.reused += 1;
                continue;
            }
            let mut bytes = Vec::new();
            let Ok(file) = fs::File::open(&path) else {
                continue;
            };
            if file.take(MAX_BYTES + 1).read_to_end(&mut bytes).is_err()
                || bytes.len() as u64 > MAX_BYTES
                || bytes.contains(&0)
            {
                continue;
            }
            let Ok(source) = String::from_utf8(bytes) else {
                continue;
            };
            let hash = format!("{:x}", Sha256::digest(source.as_bytes()));
            if !force && old.is_some_and(|f| f.hash == hash) {
                let mut reused = old.unwrap().clone();
                reused.size = size;
                reused.modified = modified;
                next.files.insert(path, reused);
                progress.reused += 1;
                continue;
            }
            let relative = key(root, &path);
            let language = crate::language_for_path(&path);
            let mut entries = Vec::new();
            let symbols = if language == "php" {
                match scanner::scan_laravel(&source, cancel, composer.laravel) {
                    Ok(symbols) => {
                        // Build once per parsed PHP file. Repeated nth(line)
                        // scans from byte zero for every symbol in large files.
                        let source_lines = source.lines().collect::<Vec<_>>();
                        for symbol in &symbols.symbols {
                            if matches!(
                                symbol.kind,
                                scanner::Kind::Import | scanner::Kind::Attribute
                            ) {
                                continue;
                            }
                            entries.push(IndexEntry {
                                path: relative.clone(),
                                kind: EntryKind::SymbolDef,
                                language: language.clone(),
                                symbol: Some(symbol.qualified.clone()),
                                line: symbol.line as u64,
                                col: symbol.column as u64,
                                body: source_lines
                                    .get(symbol.line)
                                    .copied()
                                    .unwrap_or("")
                                    .to_owned(),
                            });
                        }
                        symbols
                    }
                    Err(e) => {
                        if cancel.load(Ordering::Relaxed) {
                            progress.cancelled = true;
                            return Ok(progress);
                        }
                        progress.warnings.push(e);
                        FileSymbols::default()
                    }
                }
            } else {
                let extracted = match language.as_str() {
                    "javascript" => Some(crate::symbols::javascript_definitions(
                        &relative, &source,
                    )),
                    "typescript" => Some(crate::symbols::typescript_definitions(
                        &relative,
                        &source,
                        path.extension().is_some_and(|s| s == "tsx"),
                    )),
                    _ => None,
                };
                if let Some(result) = extracted {
                    match result {
                        Ok(found) => entries.extend(found),
                        Err(error) => {
                            progress.warnings.push(format!("{relative}: {error}"))
                        }
                    }
                }
                FileSymbols::default()
            };
            entries.push(IndexEntry {
                path: relative,
                kind: EntryKind::File,
                language,
                symbol: None,
                line: 0,
                col: 0,
                body: source,
            });
            next.files.insert(
                path.clone(),
                CachedFile {
                    size,
                    modified,
                    hash,
                    symbols,
                },
            );
            replacements.push((path, entries));
            progress.parsed += 1;
        }
        if cancel.load(Ordering::Relaxed) {
            progress.cancelled = true;
            return Ok(progress);
        }
        let removed_count = self
            .cache
            .files
            .keys()
            .filter(|p| !next.files.contains_key(*p))
            .count();
        if replacements.is_empty()
            && removed_count == 0
            && self.cache.config == next.config
            && self.cache.version == VERSION
        {
            self.cache = next;
            return Ok(progress);
        }
        // Stage all extraction before touching the writer. Readers see old commit
        // until the single commit completes; cancellation never discards it.
        let cache_bytes =
            serde_json::to_vec(&next).map_err(std::io::Error::other)?;
        if cache_bytes.len() > 64 * 1024 * 1024 {
            return Err(std::io::Error::other(
                "symbol sidecar exceeds64MiB; previous generation retained",
            )
            .into());
        }
        // Crash between Tantivy commit and cache replacement must invalidate
        // metadata reuse on next open. Keep the marker until both are durable.
        let marker = self.cache_path.with_file_name("php-cache-commit.pending");
        let marker_file = fs::File::create(&marker)?;
        marker_file.sync_all()?;
        let result = (|| {
            if self.cache.version != VERSION {
                self.index.clear()?;
            }
            for path in self
                .cache
                .files
                .keys()
                .filter(|p| !next.files.contains_key(*p))
            {
                self.index.remove_path(&key(root, path))?;
                progress.removed += 1;
            }
            for (path, entries) in replacements {
                self.index.remove_path(&key(root, &path))?;
                for entry in entries {
                    self.index.add_entry(&entry)?;
                }
            }
            self.index.commit()
        })();
        if let Err(error) = result {
            self.index.rollback()?;
            return Err(error);
        }
        *self.hot.lock().unwrap() = next
            .files
            .iter()
            .map(|(p, f)| (p.clone(), f.symbols.clone()))
            .collect();
        self.cache = next;
        let bytes =
            serde_json::to_vec(&self.cache).map_err(std::io::Error::other)?;
        let mut temp =
            tempfile::NamedTempFile::new_in(self.cache_path.parent().unwrap())?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(&self.cache_path).map_err(|e| e.error)?;
        fs::remove_file(marker)?;
        Ok(progress)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexed_lines_nth_parity_sanity_check() {
        for source in ["", "one", "one\n", "one\r\ntwo\r\n", "café\n\nthree\rfour"]
        {
            let lines = source.lines().collect::<Vec<_>>();
            for line in 0..lines.len() + 3 {
                assert_eq!(
                    lines.get(line).copied().unwrap_or(""),
                    source.lines().nth(line).unwrap_or("")
                );
            }
        }
    }

    #[test]
    fn symbol_bodies_preserve_line_semantics_after_indexing() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("project");
        fs::create_dir(&root).unwrap();
        let source = "<?php\r\nnamespace App;\r\n// UTF-8 café\r\nclass Invoice {\r\n    public function paid() {}\r\n}\r\n";
        fs::write(root.join("Invoice.php"), source).unwrap();
        let mut engine = Engine::open(&t.path().join("index")).unwrap();
        engine
            .reconcile(&root, &BTreeSet::new(), &AtomicBool::new(false))
            .unwrap();
        let symbols = engine.snapshot();
        let expected = symbols
            .values()
            .flat_map(|file| &file.symbols)
            .filter(|symbol| {
                !matches!(
                    symbol.kind,
                    scanner::Kind::Import | scanner::Kind::Attribute
                )
            })
            .map(|symbol| {
                (
                    symbol.qualified.clone(),
                    source.lines().nth(symbol.line).unwrap_or("").to_owned(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert!(expected.len() >= 2);
        let searcher = engine.index.reader.searcher();
        let docs = searcher
            .search(
                &tantivy::query::AllQuery,
                &tantivy::collector::TopDocs::with_limit(100),
            )
            .unwrap();
        let mut checked = 0;
        for (_, address) in docs {
            let document: tantivy::TantivyDocument = searcher.doc(address).unwrap();
            if engine
                .index
                .text_value(&document, engine.index.fields.kind)
                .as_deref()
                != Some(EntryKind::SymbolDef.as_str())
            {
                continue;
            }
            let symbol = engine
                .index
                .text_value(&document, engine.index.fields.symbol)
                .unwrap();
            let body = engine
                .index
                .text_value(&document, engine.index.fields.body)
                .unwrap();
            assert_eq!(body, expected[&symbol]);
            checked += 1;
        }
        assert_eq!(checked, expected.len());
    }

    #[test]
    #[ignore = "hosted line-body extraction measurement, not an indexing SLA"]
    fn php_symbol_line_lookup_baseline() {
        use std::{hint::black_box, time::Instant};
        let source = (0..12000)
            .map(|i| format!("    public function synthetic_{i}() {{}}\n"))
            .collect::<String>();
        let lines = (0..12000).step_by(3).collect::<Vec<_>>();
        let start = Instant::now();
        let mut before = 0usize;
        for _ in 0..3 {
            for &line in &lines {
                before +=
                    black_box(source.lines().nth(black_box(line)).unwrap_or(""))
                        .len();
            }
        }
        let before_us = start.elapsed().as_micros();
        let start = Instant::now();
        let mut after = 0usize;
        for _ in 0..3 {
            let indexed = black_box(source.lines().collect::<Vec<_>>());
            for &line in &lines {
                after +=
                    black_box(indexed.get(black_box(line)).copied().unwrap_or(""))
                        .len();
            }
        }
        let after_us = start.elapsed().as_micros();
        assert_eq!(before, after);
        assert!(source.len() < MAX_BYTES as usize);
        println!(
            "PHP_LINE_BODY bytes={} lines=12000 lookups={} repetitions=3 repeated_nth_us={} indexed_lines_us={} equal_body_bytes=true (lookup stage only; not full indexing)",
            source.len(),
            lines.len(),
            before_us,
            after_us
        );
    }

    #[test]
    fn old_cache_version_reparses_unchanged_source() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("project");
        let dir = t.path().join("index");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("Invoice.php"), "<?php class Invoice {}").unwrap();
        let cancel = AtomicBool::new(false);
        let mut engine = Engine::open(&dir).unwrap();
        engine.reconcile(&root, &BTreeSet::new(), &cancel).unwrap();
        engine.cache.version = 2;
        let progress = engine.reconcile(&root, &BTreeSet::new(), &cancel).unwrap();
        assert_eq!(progress.parsed, 1);
        assert_eq!(progress.reused, 0);
        assert_eq!(engine.cache.version, VERSION);
    }
    #[test]
    fn laravel_aliases_follow_edits_cache_and_composer_changes() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("project");
        let dir = t.path().join("index");
        fs::create_dir(&root).unwrap();
        let composer = root.join("composer.json");
        let model = root.join("Invoice.php");
        fs::write(&composer, r#"{"require":{"laravel/framework":"^12"}}"#).unwrap();
        fs::write(&model,"<?php class Invoice extends \\Illuminate\\Database\\Eloquent\\Model {public function scopePaid($q){}} ").unwrap();
        let cancel = AtomicBool::new(false);
        let mut engine = Engine::open(&dir).unwrap();
        engine.reconcile(&root, &BTreeSet::new(), &cancel).unwrap();
        assert!(
            engine
                .symbols("paid", 20)
                .iter()
                .any(|(_, s)| s.name == "paid"
                    && s.target.as_deref() == Some("Invoice::scopePaid"))
        );
        engine.reconcile(&root, &BTreeSet::new(), &cancel).unwrap();
        assert!(
            engine
                .snapshot()
                .values()
                .any(|f| f.symbols.iter().any(|s| s.name == "paid"))
        );
        fs::write(&model,"<?php class Invoice extends \\Illuminate\\Database\\Eloquent\\Model {public function scopeDue($q){}} ").unwrap();
        engine
            .reconcile(&root, &BTreeSet::from([model]), &cancel)
            .unwrap();
        assert!(
            !engine
                .snapshot()
                .values()
                .any(|f| f.symbols.iter().any(|s| s.name == "paid"))
        );
        assert!(
            engine
                .snapshot()
                .values()
                .any(|f| f.symbols.iter().any(|s| s.name == "due"))
        );
        fs::write(&composer, "{}").unwrap();
        engine.reconcile(&root, &BTreeSet::new(), &cancel).unwrap();
        assert!(
            !engine
                .snapshot()
                .values()
                .any(|f| f.symbols.iter().any(|s| s.name == "due"))
        );
    }
    #[test]
    fn changes_deletion_rename_cache_reload_and_cancel() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("project");
        let dir = t.path().join("index");
        fs::create_dir(&root).unwrap();
        let a = root.join("A.php");
        fs::write(&a, "<?php namespace App; class Alpha {}").unwrap();
        let mut e = Engine::open(&dir).unwrap();
        let cancel = AtomicBool::new(false);
        assert_eq!(
            e.reconcile(&root, &BTreeSet::new(), &cancel)
                .unwrap()
                .parsed,
            1
        );
        assert_eq!(e.symbols("Alpha", 10).len(), 1);
        assert_eq!(
            e.reconcile(&root, &BTreeSet::new(), &cancel)
                .unwrap()
                .parsed,
            0
        );
        // Equal length explicit change must hash even if metadata is ambiguous.
        fs::write(&a, "<?php namespace App; class Bravo {}").unwrap();
        e.reconcile(&root, &BTreeSet::from([a.clone()]), &cancel)
            .unwrap();
        assert!(e.symbols("Alpha", 10).is_empty());
        assert_eq!(e.symbols("Bravo", 10).len(), 1);
        let b = root.join("B.php");
        fs::rename(&a, &b).unwrap();
        let p = e
            .reconcile(&root, &BTreeSet::from([a, b.clone()]), &cancel)
            .unwrap();
        assert_eq!(p.removed, 1);
        assert_eq!(e.symbols("Bravo", 10)[0].0, b);
        cancel.store(true, Ordering::Relaxed);
        fs::remove_file(&b).unwrap();
        assert!(
            e.reconcile(&root, &BTreeSet::new(), &cancel)
                .unwrap()
                .cancelled
        );
        assert_eq!(e.symbols("Bravo", 10).len(), 1);
        drop(e);
        let mut e = Engine::open(&dir).unwrap();
        assert_eq!(e.symbols("Bravo", 10).len(), 1);
        cancel.store(false, Ordering::Relaxed);
        e.reconcile(&root, &BTreeSet::new(), &cancel).unwrap();
        assert!(e.symbols("Bravo", 10).is_empty());
    }
    #[test]
    fn composer_edit_and_directory_rename_end_to_end() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("p");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("composer.json"),
            r#"{"autoload":{"psr-4":{"App\\":"src"}}}"#,
        )
        .unwrap();
        let file = root.join("src/A.php");
        std::fs::write(
            &file,
            "<?php namespace App; use Vendor\\Thing; #[Entity] class A {}",
        )
        .unwrap();
        let mut engine = Engine::open(&t.path().join("index")).unwrap();
        let cancel = AtomicBool::new(false);
        engine.reconcile(&root, &BTreeSet::new(), &cancel).unwrap();
        let c = Composer::load(&root);
        assert_eq!(c.resolve("App\\A")[0], file);
        assert!(
            engine
                .index
                .symbol_prefix("App\\Thing", "php", 10)
                .unwrap()
                .is_empty()
        );
        std::fs::rename(root.join("src"), root.join("lib")).unwrap();
        std::fs::write(
            root.join("composer.json"),
            r#"{"autoload":{"psr-4":{"App\\":"lib"}}}"#,
        )
        .unwrap();
        let progress = engine
            .reconcile(
                &root,
                &BTreeSet::from([
                    root.join("composer.json"),
                    root.join("src"),
                    root.join("lib"),
                ]),
                &cancel,
            )
            .unwrap();
        assert_eq!(progress.removed, 1);
        assert_eq!(
            engine
                .symbols("A", 10)
                .iter()
                .find(|(_, s)| s.kind == scanner::Kind::Class)
                .unwrap()
                .0,
            root.join("lib/A.php")
        );
        assert_eq!(
            Composer::load(&root).resolve("App\\A")[0],
            root.join("lib/A.php")
        );
    }
    #[test]
    fn interrupted_commit_marker_invalidates_warm_cache() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("project");
        let dir = t.path().join("index");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("A.php"), "<?php class Before {}").unwrap();
        let cancel = AtomicBool::new(false);
        let mut engine = Engine::open(&dir).unwrap();
        engine.reconcile(&root, &BTreeSet::new(), &cancel).unwrap();
        drop(engine);
        fs::write(dir.join("php-cache-commit.pending"), "").unwrap();
        let mut engine = Engine::open(&dir).unwrap();
        assert!(engine.symbols("Before", 10).is_empty());
        assert_eq!(
            engine
                .reconcile(&root, &BTreeSet::new(), &cancel)
                .unwrap()
                .parsed,
            1
        );
        assert_eq!(engine.symbols("Before", 10).len(), 1);
        assert!(!dir.join("php-cache-commit.pending").exists());
    }
    #[test]
    fn composer_vendor_ignores_and_old_cache_version() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path().join("p");
        let d = t.path().join("i");
        fs::create_dir_all(r.join("vendor/a/b/src")).unwrap();
        fs::create_dir_all(r.join("node_modules")).unwrap();
        fs::write(r.join(".gitignore"), "vendor/\nignored.php\n").unwrap();
        fs::write(r.join("ignored.php"), "<?php class Ignored{}").unwrap();
        fs::write(r.join("node_modules/A.php"), "<?php class Node{}").unwrap();
        fs::write(
            r.join("vendor/a/b/composer.json"),
            r#"{"autoload":{"psr-4":{"Vendor\\":"src"}}}"#,
        )
        .unwrap();
        fs::write(r.join("vendor/a/b/src/Yes.php"), "<?php class Yes{}").unwrap();
        let mut e = Engine::open(&d).unwrap();
        e.reconcile(&r, &BTreeSet::new(), &AtomicBool::new(false))
            .unwrap();
        assert_eq!(e.symbols("Yes", 10).len(), 1);
        assert!(e.symbols("Ignored", 10).is_empty());
        assert!(e.symbols("Node", 10).is_empty());
        drop(e);
        fs::write(
            d.join("php-symbol-cache-v1.json"),
            r#"{"version":0,"config":"","files":{}}"#,
        )
        .unwrap();
        let mut e = Engine::open(&d).unwrap();
        assert_eq!(
            e.reconcile(&r, &BTreeSet::new(), &AtomicBool::new(false))
                .unwrap()
                .parsed,
            1
        );
    }
}
