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
    core: Mutex<Vec<lapce_rpc::core::WeakCoreRpcHandler>>,
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
                owner.core.lock().unwrap().retain(|core| {
                    core.notify(lapce_rpc::core::CoreNotification::PhpFramework {
                        laravel: composer.laravel,
                    })
                });
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
        Self::start_with_core(root, cache, None)
    }
    pub fn start_with_core(
        root: PathBuf,
        cache: PathBuf,
        core: Option<lapce_rpc::core::CoreRpcHandler>,
    ) -> Self {
        let original_root = root.clone();
        let root = root.canonicalize().unwrap_or(root);
        let mut owners = OWNERS.get_or_init(Default::default).lock().unwrap();
        if let Some(owner) = owners.get(&root).and_then(Weak::upgrade) {
            if let Some(core) = core {
                let laravel = owner.composer.lock().unwrap().laravel;
                core.notification(lapce_rpc::core::CoreNotification::PhpFramework {
                    laravel,
                });
                let weak = core.downgrade();
                let mut observers = owner.core.lock().unwrap();
                if !observers.iter().any(|entry| entry.same(&weak)) {
                    observers.push(weak);
                }
            }
            return Self(owner);
        }
        let (tx, rx) = unbounded::<Vec<PathBuf>>();
        let cancel = Arc::new(AtomicBool::new(false));
        let epoch = Arc::new(AtomicU64::new(0));
        let snapshot = Arc::new(Mutex::new(Snapshot {
            state: "opening".into(),
            ..Default::default()
        }));
        let composer = lapce_index::php::composer::Composer::load(&root);
        if let Some(core) = &core {
            core.notification(lapce_rpc::core::CoreNotification::PhpFramework {
                laravel: composer.laravel,
            });
        }
        let owner = Arc::new(Owner {
            core: Mutex::new(core.iter().map(|core| core.downgrade()).collect()),
            composer: Mutex::new(composer),
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
    pub fn completion_query(
        &self,
        token: &str,
        kinds: &[String],
        container: Option<&str>,
        static_only: Option<bool>,
        allow_nonpublic: bool,
        limit: usize,
    ) -> (String, Vec<lapce_rpc::proxy::PhpIndexSymbol>) {
        let mut owners = std::collections::BTreeSet::new();
        if let Some(container) = container {
            owners.insert(container.to_owned());
            let state = self.0.snapshot.lock().unwrap();
            let mut frontier = vec![container.to_owned()];
            let mut depth = 0;
            while let Some(owner) = frontier.pop() {
                depth += 1;
                if depth > 128 {
                    break;
                }
                for symbol in state
                    .files
                    .values()
                    .flat_map(|f| &f.symbols)
                    .filter(|s| s.qualified == owner)
                {
                    for related in &symbol.related {
                        if owners.insert(related.clone()) {
                            frontier.push(related.clone());
                        }
                    }
                }
            }
        }
        self.query_filtered(token, limit, |s| {
            let kind = format!("{:?}", s.kind).to_lowercase();
            (kinds.is_empty() || kinds.contains(&kind))
                && match (container, s.container.as_deref()) {
                    (Some(_), Some(owner)) => owners.contains(owner),
                    (None, None) => true,
                    _ => false,
                }
                && static_only.is_none_or(|required| {
                    s.is_static == required
                        || required && kind == "constant"
                        || s.attributes.iter().any(|a| a == "laravel:local-scope")
                })
                && (s.visibility.as_deref().is_none_or(|v| v == "public")
                    || allow_nonpublic
                        && (s.container.as_deref() == container
                            || s.visibility.as_deref() != Some("private")))
        })
    }
    fn query_filtered(
        &self,
        prefix: &str,
        limit: usize,
        accept: impl Fn(&lapce_index::php::scanner::Symbol) -> bool,
    ) -> (String, Vec<lapce_rpc::proxy::PhpIndexSymbol>) {
        let state = self.0.snapshot.lock().unwrap();
        let prefix = prefix.to_lowercase();
        let mut matches = state
            .files
            .iter()
            .flat_map(|(path, file)| {
                file.symbols
                    .iter()
                    .filter(|s| {
                        accept(s)
                            && (subsequence(&s.name, &prefix)
                                || subsequence(&s.qualified, &prefix))
                    })
                    .map(move |s| (path, s))
            })
            .collect::<Vec<_>>();
        matches.sort_by_cached_key(|(_, s)| {
            (
                !s.name.to_lowercase().starts_with(&prefix),
                s.name.len(),
                s.name.to_lowercase(),
            )
        });
        let symbols = matches
            .into_iter()
            .take(limit.min(1000))
            .map(|(path, s)| lapce_rpc::proxy::PhpIndexSymbol {
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
            .collect();
        (state.state.clone(), symbols)
    }
    pub fn query(
        &self,
        prefix: &str,
        limit: usize,
    ) -> (String, Vec<lapce_rpc::proxy::PhpIndexSymbol>) {
        self.query_filtered(prefix, limit, |_| true)
    }
}
// Candidate prefilter only. Final matching/ranking remains Nucleo in the app.
fn subsequence(text: &str, pattern: &str) -> bool {
    let mut chars = text.chars().flat_map(char::to_lowercase);
    pattern
        .chars()
        .flat_map(char::to_lowercase)
        .all(|c| chars.by_ref().any(|t| t == c))
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
    fn framework_notifications_refresh_from_composer_and_prune_dead_observers() {
        use lapce_rpc::core::CoreRpc;
        use lapce_rpc::core::{CoreNotification, CoreRpcHandler};
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let core = CoreRpcHandler::new();
        let manager = Manager::start_with_core(
            root.clone(),
            t.path().join("cache"),
            Some(core.clone()),
        );
        let received = core.rx().recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            matches!(received,CoreRpc::Notification(n) if matches!(*n,CoreNotification::PhpFramework {laravel:false}))
        );
        std::fs::write(
            root.join("composer.lock"),
            r#"{"packages":[{"name":"laravel/framework"}]}"#,
        )
        .unwrap();
        manager.changes().changed(vec![root.join("composer.lock")]);
        let received = core.rx().recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            matches!(received,CoreRpc::Notification(n) if matches!(*n,CoreNotification::PhpFramework {laravel:true}))
        );
        assert!(lapce_index::php::events::relevant(
            &root,
            &manager.0.composer.lock().unwrap(),
            &root.join("vendor/composer/installed.json")
        ));
        drop(core);
        manager.changes().changed(vec![root.join("composer.lock")]);
        assert!(manager.0.core.lock().unwrap().is_empty());
    }
    #[test]
    #[ignore]
    fn native_50k_proxy_baseline() {
        use lapce_index::php::scanner::{Kind, Symbol};
        let (tx, _rx) = unbounded();
        let snapshot = Snapshot {
            state: "ready: synthetic".into(),
            generation: 1,
            files: BTreeMap::from([(
                "vendor/fixture.php".into(),
                FileSymbols {
                    malformed: false,
                    symbols: (0..50000)
                        .map(|i| Symbol {
                            name: format!("Model{i}"),
                            qualified: format!("Vendor\\Package{i}\\Model{i}"),
                            kind: Kind::Class,
                            namespace: format!("Vendor\\Package{i}"),
                            container: None,
                            visibility: None,
                            is_static: false,
                            alias: None,
                            target: None,
                            attributes: vec![],
                            related: vec![],
                            byte: 0,
                            end: 0,
                            line: 0,
                            column: 0,
                        })
                        .collect(),
                },
            )]),
        };
        let owner = Arc::new(Owner {
            core: Mutex::new(vec![]),
            root: PathBuf::new(),
            composer: Mutex::new(Default::default()),
            tx,
            cancel: Arc::new(AtomicBool::new(false)),
            epoch: Arc::new(AtomicU64::new(0)),
            snapshot: Arc::new(Mutex::new(snapshot)),
        });
        let manager = Manager(owner);
        for token in ["Model49", ""] {
            let mut samples = vec![];
            for _ in 0..5 {
                let start = std::time::Instant::now();
                let (_, rows) = manager.completion_query(
                    token,
                    &["class".into()],
                    None,
                    None,
                    false,
                    1000,
                );
                assert!(!rows.is_empty());
                samples.push(start.elapsed().as_secs_f64() * 1000.);
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "native50k proxy token={token:?} min_ms={:.3} median_ms={:.3} no_lsp=true",
                samples[0], samples[2]
            );
        }
        {
            let mut snapshot = manager.0.snapshot.lock().unwrap();
            let symbols = &mut snapshot
                .files
                .get_mut(&PathBuf::from("vendor/fixture.php"))
                .unwrap()
                .symbols;
            for i in 0..64 {
                let mut s = symbols[i].clone();
                s.qualified = format!("Depth{i}");
                s.name = s.qualified.clone();
                s.related = if i < 63 {
                    vec![format!("Depth{}", i + 1)]
                } else {
                    vec![]
                };
                symbols.push(s);
            }
            let mut member = symbols[0].clone();
            member.kind = Kind::Method;
            member.name = "inherited".into();
            member.qualified = "Depth63::inherited".into();
            member.container = Some("Depth63".into());
            symbols.push(member);
        }
        let mut samples = vec![];
        for _ in 0..5 {
            let start = std::time::Instant::now();
            let (_, rows) = manager.completion_query(
                "",
                &["method".into()],
                Some("Depth0"),
                Some(false),
                true,
                1000,
            );
            assert_eq!(rows.len(), 1);
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "native50k proxy depth64 min_ms={:.3} median_ms={:.3}",
            samples[0], samples[2]
        );
    }
    #[test]
    fn completion_filters_before_limit_and_inherits_without_parent_private() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("p");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("A.php"),"<?php namespace App; trait Shared {public function traitMethod(){}} class Base {protected function inherited(){} private function secret(){}} class Child extends Base {use Shared; private function own(){}} ").unwrap();
        let manager = Manager::start(root, t.path().join("cache"));
        wait_ready(&manager);
        let (_, symbols) = manager.completion_query(
            "",
            &["method".into()],
            Some("App\\Child"),
            Some(false),
            true,
            100,
        );
        for name in ["traitMethod", "inherited", "own"] {
            assert!(
                symbols.iter().any(|s| s.name == name),
                "{name}: {symbols:?}"
            );
        }
        assert!(!symbols.iter().any(|s| s.name == "secret"));
        let (_, symbols) = manager.completion_query(
            "inher",
            &["method".into()],
            Some("App\\Child"),
            Some(false),
            true,
            1,
        );
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "inherited");
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
