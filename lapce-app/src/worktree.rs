//! Git worktree helpers (UI-free). The branch name and sibling path are
//! derived from a unix timestamp, so they are charset-safe by
//! construction; the actual `git worktree add` runs in the window tab.

use std::path::{Path, PathBuf};

/// Branch name for an ad-hoc worktree.
pub fn worktree_branch_name(unix: u64) -> String {
    format!("worktree/{unix}")
}

/// Sibling directory for the new worktree:
/// `<parent>/<dir-name>-worktree-<unix>`. None when the root has no
/// usable directory name (e.g. a filesystem root). Non-ASCII directory
/// names pass through unchanged.
pub fn worktree_path(repo_root: &Path, unix: u64) -> Option<PathBuf> {
    let name = repo_root.file_name()?;
    Some(
        repo_root
            .parent()?
            .join(format!("{}-worktree-{unix}", name.to_string_lossy())),
    )
}

/// Clip command stderr for an alert: 400 bytes on a char boundary.
pub fn clip_stderr(stderr: &str) -> String {
    const CAP: usize = 400;
    let trimmed = stderr.trim();
    if trimmed.len() <= CAP {
        return trimmed.to_string();
    }
    let mut end = CAP;
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[output clipped]", &trimmed[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_name_is_charset_safe_by_construction() {
        let name = worktree_branch_name(1_729_000_000);
        assert!(name.starts_with("worktree/"));
        assert!(name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '/'));
    }

    #[test]
    fn path_is_a_sibling_and_keeps_non_ascii_names() {
        let root = Path::new("/home/dev/日本語-project");
        let path = worktree_path(root, 42).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/home/dev/日本語-project-worktree-42")
        );
        // A filesystem root has no usable name.
        assert_eq!(worktree_path(Path::new("/"), 42), None);
    }

    #[test]
    fn clip_stderr_respects_cap_and_boundaries() {
        assert_eq!(clip_stderr("  fatal: exists  "), "fatal: exists");
        let big = "日".repeat(200); // 600 bytes, 3-byte chars
        let out = clip_stderr(&big);
        assert!(out.ends_with("[output clipped]"));
        let clipped_len = out.len() - "\n[output clipped]".len();
        assert!(clipped_len <= 400);
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
    }
}
