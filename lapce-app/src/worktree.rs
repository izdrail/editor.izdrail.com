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

/// One entry from `git worktree list --porcelain`.
#[derive(Clone, Debug, PartialEq)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    /// Branch short name; None for a detached HEAD or a bare repo root.
    pub branch: Option<String>,
}

/// Parse `git worktree list --porcelain` (version 1): blank-line
/// separated records, one attribute per line. Paths containing
/// unusual bytes are C-style quoted (core.quotePath) and unquoted
/// here. Unknown attributes are ignored; records without a usable
/// worktree line are skipped.
pub fn parse_worktree_list(text: &str) -> Vec<WorktreeEntry> {
    let mut out = Vec::new();
    for record in text.split("\n\n") {
        let mut path: Option<String> = None;
        let mut branch: Option<String> = None;
        for line in record.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                path = Some(c_unquote(p));
            } else if let Some(b) = line.strip_prefix("branch ") {
                branch = Some(
                    b.strip_prefix("refs/heads/").unwrap_or(b).to_string(),
                );
            }
        }
        if let Some(p) = path {
            if !p.is_empty() {
                out.push(WorktreeEntry {
                    path: PathBuf::from(p),
                    branch,
                });
            }
        }
    }
    out
}

/// Undo git's C-style path quoting: surrounding quotes with \\, \",
/// \n, \t and octal \NNN escapes. Unquoted input is returned as-is;
/// undecodable bytes fall back to lossy UTF-8.
fn c_unquote(s: &str) -> String {
    if !(s.starts_with('"') && s.ends_with('"') && s.len() >= 2) {
        return s.to_string();
    }
    let inner = &s.as_bytes()[1..s.len() - 1];
    let mut out: Vec<u8> = Vec::with_capacity(inner.len());
    let mut i = 0;
    while i < inner.len() {
        if inner[i] == b'\\' && i + 1 < inner.len() {
            match inner[i + 1] {
                b'n' => {
                    out.push(b'\n');
                    i += 2;
                }
                b't' => {
                    out.push(b'\t');
                    i += 2;
                }
                b'\\' => {
                    out.push(b'\\');
                    i += 2;
                }
                b'"' => {
                    out.push(b'"');
                    i += 2;
                }
                b'0'..=b'7'
                    if i + 3 < inner.len()
                        && matches!(inner[i + 2], b'0'..=b'7')
                        && matches!(inner[i + 3], b'0'..=b'7') =>
                {
                    let v = (inner[i + 1] - b'0') as u32 * 64
                        + (inner[i + 2] - b'0') as u32 * 8
                        + (inner[i + 3] - b'0') as u32;
                    out.push(v as u8);
                    i += 4;
                }
                _ => {
                    out.push(inner[i]);
                    i += 1;
                }
            }
        } else {
            out.push(inner[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// How many leading worktree rows get a live dirty check per palette
/// open. Each check is one `git status --porcelain`; worktree counts
/// are small, but a bound keeps a pathological list from stalling the
/// palette.
pub const MAX_DIRTY_CHECKS: usize = 8;

/// Dirty check for one worktree: any `git status --porcelain` output
/// counts (modified, staged or untracked - matching what makes
/// `git worktree remove` refuse). Errors count as clean: a badge is a
/// hint, not a gate.
pub fn worktree_dirty(path: &Path) -> bool {
    std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["status", "--porcelain"])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

/// Filter text for one worktree palette row.
pub fn worktree_row_label(
    branch: Option<&str>,
    path: &Path,
    current: bool,
    dirty: bool,
) -> String {
    let branch = branch.unwrap_or("(detached)");
    let mut label = format!("{branch}  {}", path.display());
    if current {
        label.push_str("  (current)");
    }
    if dirty {
        label.push_str("  (dirty)");
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_worktree_list_records() {
        let text = "worktree /home/dev/proj\nHEAD 0123abcd\nbranch refs/heads/main\n\nworktree /home/dev/proj-wt\nHEAD 89abcdef\ndetached\n\n";
        let entries = parse_worktree_list(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, PathBuf::from("/home/dev/proj"));
        assert_eq!(entries[0].branch.as_deref(), Some("main"));
        assert_eq!(entries[1].branch, None);
    }

    #[test]
    fn parse_worktree_list_quoted_and_messy() {
        // C-style quoted non-ASCII path (core.quotePath), unknown
        // attributes, an empty path and a record with no worktree
        // line: none of these may panic or produce phantom entries.
        let text = "worktree \"/home/dev/\\346\\227\\245\\346\\234\\254-proj\"\nHEAD abc\nbranch refs/heads/feat/x\nprunable gitdir file gone\n\nworktree \n\nHEAD dead\n";
        let entries = parse_worktree_list(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, PathBuf::from("/home/dev/日本-proj"));
        assert_eq!(entries[0].branch.as_deref(), Some("feat/x"));
        // Never panics on arbitrary bytes.
        let _ = parse_worktree_list("worktree \"unclosed\n\\zz\n\n\n");
        assert!(parse_worktree_list("").is_empty());
    }

    #[test]
    fn row_label_marks_current_dirty_and_detached() {
        assert_eq!(
            worktree_row_label(
                Some("main"),
                Path::new("/home/dev/proj"),
                true,
                false
            ),
            "main  /home/dev/proj  (current)"
        );
        assert_eq!(
            worktree_row_label(
                None,
                Path::new("/home/dev/\u{65e5}\u{672c}-proj"),
                false,
                true
            ),
            "(detached)  /home/dev/\u{65e5}\u{672c}-proj  (dirty)"
        );
        assert_eq!(
            worktree_row_label(Some("f/x"), Path::new("/p"), false, false),
            "f/x  /p"
        );
    }

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
