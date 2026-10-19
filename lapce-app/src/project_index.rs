//! Background project indexing: on workspace open, walk every file into
//! the embedded lapce-index store (docs/project-index.md, build plan
//! patch 2). The walk itself lives in lapce-index::walker; this module
//! owns the editor-side decisions and the background thread.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use floem::ext_event::create_signal_from_channel;
use floem::reactive::{RwSignal, Scope, SignalGet, SignalUpdate};
use lapce_core::directory::Directory;
use lapce_index::{IndexStats, ProjectIndex, index_workspace};
use tracing::error;

use crate::workspace::LapceWorkspace;

/// Lifecycle of the background first-open index pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndexStatus {
    Indexing { files: usize },
    Done(IndexStats),
    Failed(String),
}

/// A running (or finished) project index for one workspace.
#[derive(Clone)]
pub struct ProjectIndexHandle {
    pub index: Arc<ProjectIndex>,
    pub stats: RwSignal<IndexStatus>,
    cancel: Arc<AtomicBool>,
}

impl ProjectIndexHandle {
    /// Ask the background walk to stop. Committed batches stay
    /// searchable; the walker reports the partial stats as cancelled.
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Only local workspaces with a path can be indexed from this process.
pub fn should_index(workspace: &LapceWorkspace) -> bool {
    workspace.kind.is_local() && workspace.path.is_some()
}

/// Where the index for one workspace lives: under the editor cache
/// directory, keyed by a hash of the workspace path so projects never
/// collide and no index files are written into the user's project.
pub fn index_dir(cache_root: &Path, workspace_path: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    workspace_path.hash(&mut hasher);
    cache_root
        .join("indexes")
        .join(format!("{:016x}", hasher.finish()))
}

/// Open the on-disk index for `workspace` and spawn the background
/// first-open walk. Returns None for remote/empty workspaces and when
/// the index or the thread cannot be created (logged, never fatal).
pub fn start(cx: Scope, workspace: &Arc<LapceWorkspace>) -> Option<ProjectIndexHandle> {
    if !should_index(workspace) {
        return None;
    }
    let root = workspace.path.clone()?;
    let cache_root = Directory::cache_directory()?;
    let dir = index_dir(&cache_root, &root);
    let index = match ProjectIndex::open_or_create(&dir) {
        Ok(index) => Arc::new(index),
        Err(err) => {
            error!("failed to open project index at {dir:?}: {err}");
            return None;
        }
    };

    let stats = cx.create_rw_signal(IndexStatus::Indexing { files: 0 });
    let cancel = Arc::new(AtomicBool::new(false));

    // A scope-owned value cancels the worker when the tab scope is disposed.
    let _lifetime = cx.create_rw_signal(CancelOnDrop(cancel.clone()));
    let (tx, rx) = std::sync::mpsc::channel::<IndexStatus>();
    let updates = create_signal_from_channel(rx);
    cx.create_effect(move |_| {
        if let Some(update) = updates.get() {
            stats.set(update);
        }
    });

    let thread_index = index.clone();
    let thread_cancel = cancel.clone();
    let spawned = std::thread::Builder::new()
        .name("project index".to_owned())
        .spawn(move || {
            let result =
                index_workspace(&root, &thread_index, &thread_cancel, |files| {
                    let _ = tx.send(IndexStatus::Indexing { files });
                });
            let status = match result {
                Ok(done) => IndexStatus::Done(done),
                Err(err) => IndexStatus::Failed(err.to_string()),
            };
            let _ = tx.send(status);
        });
    if let Err(err) = spawned {
        error!("failed to spawn the project index thread: {err}");
        return None;
    }

    Some(ProjectIndexHandle {
        index,
        stats,
        cancel,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{LapceWorkspaceType, SshHost};

    fn local_workspace(path: Option<PathBuf>) -> LapceWorkspace {
        LapceWorkspace {
            kind: LapceWorkspaceType::Local,
            path,
            last_open: 0,
        }
    }

    #[test]
    fn scope_guard_drop_cancels_worker() {
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let _guard = CancelOnDrop(cancel.clone());
            assert!(!cancel.load(Ordering::Relaxed));
        }
        assert!(cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn only_local_workspaces_with_paths_are_indexed() {
        assert!(should_index(&local_workspace(Some(PathBuf::from("/tmp/proj")))));
        assert!(!should_index(&local_workspace(None)));
        let remote = LapceWorkspace {
            kind: LapceWorkspaceType::RemoteSSH(SshHost {
                user: None,
                host: "example.com".to_string(),
                port: None,
            }),
            path: Some(PathBuf::from("/srv/proj")),
            last_open: 0,
        };
        assert!(!should_index(&remote));
    }

    #[test]
    fn index_dirs_are_stable_scoped_and_distinct() {
        let cache = Path::new("/cache");
        let a = index_dir(cache, Path::new("/work/shop"));
        let a_again = index_dir(cache, Path::new("/work/shop"));
        let b = index_dir(cache, Path::new("/work/blog"));

        assert_eq!(a, a_again);
        assert_ne!(a, b);
        assert!(a.starts_with(cache.join("indexes")));
        assert!(b.starts_with(cache.join("indexes")));
    }
}
