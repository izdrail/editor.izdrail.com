//! Sole workspace-index owner, shared between local tabs in this process.
//! Cross-process ownership is enforced by Tantivy's writer lock: lock errors
//! are reported, never hidden by recreating or opening a second index.
use crossbeam_channel::{Sender, unbounded};
use lapce_index::php::{cache::Engine, scanner::FileSymbols};
use std::hash::{Hash, Hasher};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
#[derive(Default)]
pub struct Snapshot {
    pub files: BTreeMap<PathBuf, FileSymbols>,
    pub state: String,
    pub generation: u64,
}
struct Owner {
    root: PathBuf,
    composer: Mutex<lapce_index::php::composer::Composer>,
    tx: Sender<Vec<PathBuf>>,
    cancel: Arc<AtomicBool>,
    epoch: Arc<AtomicU64>,
    snapshot: Arc<Mutex<Snapshot>>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        let _ = self.tx.send(vec![]);
    }
}
#[derive(Clone)]
pub struct Manager(Arc<Owner>);
#[derive(Clone)]
pub struct Changes(Weak<Owner>);
impl Changes {
    pub fn changed(&self, paths: Vec<PathBuf>) {
        if let Some(owner) = self.0.upgrade() {
            let mut composer = owner.composer.lock().unwrap();
            let paths = paths
                .into_iter()
                .filter_map(|p| lapce_index::php::events::normalize(&p))
                .filter(|p| {
                    lapce_index::php::events::relevant(&owner.root, &composer, p)
                })
                .collect::<Vec<_>>();
            if paths.is_empty() {
                return;
            }
            if paths.iter().any(|p| {
                p.file_name().is_some_and(|n| {
                    n == "composer.json"
                        || n == "composer.lock"
                        || n == "installed.json"
                })
            }) {
                *composer = lapce_index::php::composer::Composer::load(&owner.root);
            }
            owner.epoch.fetch_add(1, Ordering::Release);
            let _ = owner.tx.send(paths);
        }
    }
}
static OWNERS: OnceLock<Mutex<HashMap<PathBuf, Weak<Owner>>>> = OnceLock::new();
pub fn index_dir(cache: &Path, root: &Path) -> PathBuf {
    // Preserve the existing app-owned index path during ownership transfer.
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hash);
    cache
        .join("indexes")
        .join(format!("{:016x}", hash.finish()))
}
impl Manager {
    pub fn start(root: PathBuf, cache: PathBuf) -> Self {
        let original_root = root.clone();
        let root = root.canonicalize().unwrap_or(root);
        let mut owners = OWNERS.get_or_init(Default::default).lock().unwrap();
        if let Some(owner) = owners.get(&root).and_then(Weak::upgrade) {
            return Self(owner);
        }
        let (tx, rx) = unbounded::<Vec<PathBuf>>();
        let cancel = Arc::new(AtomicBool::new(false));
        let epoch = Arc::new(AtomicU64::new(0));
        let snapshot = Arc::new(Mutex::new(Snapshot {
            state: "opening".into(),
            ..Default::default()
        }));
        let owner = Arc::new(Owner {
            composer: Mutex::new(lapce_index::php::composer::Composer::load(&root)),
            root: root.clone(),
            tx,
            cancel: cancel.clone(),
            epoch: epoch.clone(),
            snapshot: snapshot.clone(),
        });
        owners.insert(root.clone(), Arc::downgrade(&owner));
        let error_snapshot = snapshot.clone();
        let spawn = std::thread::Builder::new()
            .name("PHP project index".into())
            .spawn(move || {
                let dir = index_dir(&cache, &original_root);
                let mut attempts = 0;
                let mut engine = loop {
                    if cancel.load(Ordering::Acquire) {
                        return;
                    }
                    match Engine::open(&dir) {
                        Ok(engine) => break engine,
                        Err(e) => {
                            attempts += 1;
                            if attempts >= 30 {
                                snapshot.lock().unwrap().state =format!("failed: {e}; waiting for relevant change to retry");
                                if rx.recv().is_err(){return;}attempts=0;
                            }
                            snapshot.lock().unwrap().state =
                                format!("waiting for index: {e}");
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    }
                };
                {
                    let mut s = snapshot.lock().unwrap();
                    s.files = engine.snapshot();
                    s.state = "indexing".into();
                }
                let mut dirty = BTreeSet::new();
                loop {
                    if cancel.load(Ordering::Acquire) {
                        return;
                    }
                    let started = epoch.load(Ordering::Acquire);
                    snapshot.lock().unwrap().state = "indexing".into();
                    match engine.reconcile(&root, &dirty, &cancel) {
                        Ok(progress) if !progress.cancelled => {
                            let mut s = snapshot.lock().unwrap();
                            s.files = engine.snapshot();
                            s.generation += 1;
                            s.state = format!(
                                "ready: {} parsed, {} reused, {} removed{}",
                                progress.parsed,
                                progress.reused,
                                progress.removed,
                                if progress.warnings.is_empty() {
                                    String::new()
                                } else {
                                    format!(", {} warnings", progress.warnings.len())
                                }
                            );
                        }
                        Ok(_) => return,
                        Err(e) => {
                            snapshot.lock().unwrap().state = format!("failed: {e}")
                        }
                    }
                    dirty.clear();
                    // Events received during a pass are retained and reconciled, not
                    // dropped. Debounce the burst, not a fixed polling loop.
                    if epoch.load(Ordering::Acquire) == started {
                        match rx.recv() {
                            Ok(paths) => dirty.extend(paths),
                            Err(_) => return,
                        }
                    }
                    while let Ok(paths) = rx.recv_timeout(Duration::from_millis(120))
                    {
                        dirty.extend(paths);
                        if cancel.load(Ordering::Acquire) {
                            return;
                        }
                    }
                    if cancel.load(Ordering::Acquire) {
                        return;
                    }
                }
            });
        if let Err(error) = spawn {
            error_snapshot.lock().unwrap().state =
                format!("failed to start worker: {error}");
        }
        Self(owner)
    }
    pub fn changes(&self) -> Changes {
        Changes(Arc::downgrade(&self.0))
    }
    pub fn query(
        &self,
        prefix: &str,
        limit: usize,
    ) -> (String, Vec<lapce_rpc::proxy::PhpIndexSymbol>) {
        let state = self.0.snapshot.lock().unwrap();
        let prefix = prefix.to_lowercase();
        let symbols = state
            .files
            .iter()
            .flat_map(|(path, file)| {
                file.symbols
                    .iter()
                    .filter(|s| {
                        s.name.to_lowercase().starts_with(&prefix)
                            || s.qualified.to_lowercase().starts_with(&prefix)
                    })
                    .map(move |s| lapce_rpc::proxy::PhpIndexSymbol {
                        path: path.clone(),
                        name: s.name.clone(),
                        qualified: s.qualified.clone(),
                        kind: format!("{:?}", s.kind).to_lowercase(),
                        namespace: s.namespace.clone(),
                        container: s.container.clone(),
                        visibility: s.visibility.clone(),
                        is_static: s.is_static,
                        alias: s.alias.clone(),
                        target: s.target.clone(),
                        attributes: s.attributes.clone(),
                        byte: s.byte,
                        end: s.end,
                        line: s.line,
                        column: s.column,
                    })
            })
            .take(limit.min(1000))
            .collect();
        (state.state.clone(), symbols)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wait_ready(manager: &Manager) {
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        while std::time::Instant::now() < deadline {
            if manager.query("", 1).0.starts_with("ready:") {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("worker state: {}", manager.query("", 1).0);
    }
    #[test]
    fn restart_and_ignored_watcher_pipeline() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("p");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("A.php"), "<?php class Alpha {}").unwrap();
        std::fs::write(root.join(".gitignore"), "ignored.php\n").unwrap();
        let one = Manager::start(root.clone(), t.path().join("cache"));
        wait_ready(&one);
        drop(one);
        let two = Manager::start(root.clone(), t.path().join("cache"));
        wait_ready(&two);
        assert_eq!(two.query("Alpha", 10).1.len(), 1);
        let generation = two.0.snapshot.lock().unwrap().generation;
        std::fs::write(root.join("ignored.php"), "<?php class Ignored {}").unwrap();
        two.changes().changed(vec![root.join("ignored.php")]);
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(generation, two.0.snapshot.lock().unwrap().generation);
        std::fs::write(root.join("A.php"), "<?php class Bravo {}").unwrap();
        two.changes().changed(vec![root.join("A.php")]);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline
            && two.query("Bravo", 10).1.is_empty()
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(two.query("Bravo", 10).1.len(), 1);
        assert!(two.query("Alpha", 10).1.is_empty());
        let dir = root.join("module");
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("Gone.php"), "<?php class Gone {}").unwrap();
        two.changes().changed(vec![dir.clone()]);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline
            && two.query("Gone", 10).1.is_empty()
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(two.query("Gone", 10).1.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
        two.changes().changed(vec![dir]);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline
            && !two.query("Gone", 10).1.is_empty()
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(two.query("Gone", 10).1.is_empty());
    }
    #[test]
    fn stable_directory_and_same_process_owner() {
        let root =
            std::env::temp_dir().join(format!("php-owner-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let cache = root.join("cache");
        assert_eq!(index_dir(&cache, &root), index_dir(&cache, &root));
        let one = Manager::start(root.clone(), cache.clone());
        let two = Manager::start(root, cache);
        assert!(Arc::ptr_eq(&one.0, &two.0));
    }
}
