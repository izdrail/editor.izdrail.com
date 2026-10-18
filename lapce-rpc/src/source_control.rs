use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DiffInfo {
    pub head: String,
    pub branches: Vec<String>,
    pub tags: Vec<String>,
    pub diffs: Vec<FileDiff>,
    /// Changes staged in the index (HEAD tree -> index), kept
    /// separate from the workdir list so the panel can show them in
    /// their own section.
    pub staged_diffs: Vec<FileDiff>,
    /// Commits the checked-out branch is ahead of its upstream.
    /// 0 when the branch has no upstream or HEAD is detached.
    pub ahead: usize,
    /// Commits the checked-out branch is behind its upstream.
    pub behind: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FileDiff {
    Modified(PathBuf),
    Added(PathBuf),
    Deleted(PathBuf),
    Renamed(PathBuf, PathBuf),
    /// Unmerged after a merge/pull: listed but refused by commit.
    Conflicted(PathBuf),
}

impl FileDiff {
    pub fn path(&self) -> &PathBuf {
        match &self {
            FileDiff::Modified(p)
            | FileDiff::Added(p)
            | FileDiff::Deleted(p)
            | FileDiff::Renamed(_, p)
            | FileDiff::Conflicted(p) => p,
        }
    }

    pub fn kind(&self) -> FileDiffKind {
        match self {
            FileDiff::Modified(_) => FileDiffKind::Modified,
            FileDiff::Added(_) => FileDiffKind::Added,
            FileDiff::Deleted(_) => FileDiffKind::Deleted,
            FileDiff::Renamed(_, _) => FileDiffKind::Renamed,
            FileDiff::Conflicted(_) => FileDiffKind::Conflicted,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDiffKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Conflicted,
}
