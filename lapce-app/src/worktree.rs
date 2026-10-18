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

/// Toast body for `git stash push`. Git reports the outcome on
/// stdout ("Saved working directory..." or "No local changes to
/// save") and failures on stderr.
pub fn stash_result_message(
    success: bool,
    stdout: &str,
    stderr: &str,
) -> Result<String, String> {
    let stdout = stdout.trim();
    if success {
        if !stdout.is_empty() {
            return Ok(stdout.to_string());
        }
        let detail = clip_stderr(stderr);
        return Ok(if detail.is_empty() {
            "Stashed.".to_string()
        } else {
            format!("Stashed. {detail}")
        });
    }
    Err(format!("git stash failed:\n{}", clip_stderr(stderr)))
}

/// Toast body for `git merge --abort`: silent on success, fatal text
/// on stderr when no merge is in progress.
pub fn merge_abort_result_message(
    success: bool,
    stdout: &str,
    stderr: &str,
) -> Result<String, String> {
    let stdout = stdout.trim();
    if success {
        if !stdout.is_empty() {
            return Ok(stdout.to_string());
        }
        return Ok("Merge aborted.".to_string());
    }
    Err(format!(
        "git merge --abort failed:\n{}",
        clip_stderr(stderr)
    ))
}

/// `git add -- <paths>` or `git reset -q -- <paths>`: stage or
/// unstage paths. Silent on success (the fs watcher refreshes the
/// panel); errors carry clipped stderr for a toast.
pub fn stage_paths(
    root: &Path,
    paths: &[PathBuf],
    stage: bool,
) -> Result<(), String> {
    let verb = if stage { "add" } else { "reset" };
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C").arg(root);
    if stage {
        cmd.arg("add").arg("--");
    } else {
        cmd.arg("reset").arg("-q").arg("--");
    }
    for path in paths {
        cmd.arg(path);
    }
    let out = cmd
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("Could not run git: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    Err(format!("git {verb} failed:\n{}", clip_stderr(&stderr)))
}

/// One parsed hunk from single-file `git diff` output.
pub struct DiffHunk {
    /// 1-based start line in the new (workdir) file.
    pub new_start: usize,
    /// Number of new-file lines the hunk covers (0 for pure
    /// deletions).
    pub new_len: usize,
    /// The full hunk text including its `@@` header line.
    pub text: String,
}

/// Split single-file `git diff` output into the file header
/// (everything before the first `@@`) and its hunks. None when the
/// diff has no hunks (unchanged file, untracked path, binary).
pub fn split_file_diff(diff_text: &str) -> Option<(String, Vec<DiffHunk>)> {
    let mut header = String::new();
    let mut hunks = Vec::new();
    let mut current: Option<DiffHunk> = None;
    for line in diff_text.lines() {
        if line.starts_with("@@ ") {
            if let Some(hunk) = current.take() {
                hunks.push(hunk);
            }
            let (new_start, new_len) = parse_hunk_header(line)?;
            current = Some(DiffHunk {
                new_start,
                new_len,
                text: format!("{line}\n"),
            });
        } else if let Some(hunk) = current.as_mut() {
            hunk.text.push_str(line);
            hunk.text.push('\n');
        } else {
            header.push_str(line);
            header.push('\n');
        }
    }
    if let Some(hunk) = current.take() {
        hunks.push(hunk);
    }
    if hunks.is_empty() {
        return None;
    }
    Some((header, hunks))
}

/// Parse the new-file range out of an `@@ -a[,b] +c[,d] @@` header.
fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let plus = line.find(" +")? + 2;
    let rest = &line[plus..];
    let end = rest.find(' ').unwrap_or(rest.len());
    let range = &rest[..end];
    match range.split_once(',') {
        Some((start, len)) => Some((start.parse().ok()?, len.parse().ok()?)),
        None => Some((range.parse().ok()?, 1)),
    }
}

/// Index of the hunk covering `line` (1-based, new-file line
/// numbers). A pure-deletion hunk owns no new-file lines, so it
/// matches the line before or after its position.
pub fn hunk_at_line(hunks: &[DiffHunk], line: usize) -> Option<usize> {
    hunks.iter().position(|hunk| {
        if hunk.new_len == 0 {
            line == hunk.new_start || line == hunk.new_start + 1
        } else {
            line >= hunk.new_start && line < hunk.new_start + hunk.new_len
        }
    })
}

/// Rebuild a one-file patch carrying only the given hunk.
pub fn build_single_hunk_patch(header: &str, hunk: &DiffHunk) -> String {
    let mut patch = String::with_capacity(header.len() + hunk.text.len());
    patch.push_str(header);
    patch.push_str(&hunk.text);
    patch
}

/// Stage the workdir hunk covering `line` (1-based) for `rel_path`:
/// `git diff -- <path>`, pick the hunk, feed a single-hunk patch to
/// `git apply --cached`. Ok(false) when no hunk covers the line (or
/// the file has no unstaged changes).
pub fn stage_hunk_at_line(
    root: &Path,
    rel_path: &Path,
    line: usize,
) -> Result<bool, String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("diff")
        .arg("--")
        .arg(rel_path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("Could not run git: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git diff failed:\n{}", clip_stderr(&stderr)));
    }
    let diff_text = String::from_utf8_lossy(&out.stdout);
    let Some((header, hunks)) = split_file_diff(&diff_text) else {
        return Ok(false);
    };
    let Some(index) = hunk_at_line(&hunks, line) else {
        return Ok(false);
    };
    let patch = build_single_hunk_patch(&header, &hunks[index]);
    let mut child = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("apply")
        .arg("--cached")
        .arg("-")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not run git: {e}"))?;
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| "Could not open git apply stdin.".to_string())?
        .write_all(patch.as_bytes())
        .map_err(|e| format!("Could not write to git apply: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("Could not run git: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "git apply --cached failed:\n{}",
            clip_stderr(&stderr)
        ));
    }
    Ok(true)
}

/// One line of `git blame` output.
pub struct BlameLine {
    /// Full commit SHA (all zeros for uncommitted lines).
    pub sha: String,
    pub author: String,
    /// Author time as epoch seconds.
    pub author_time: i64,
    pub text: String,
}

/// Parse `git blame --line-porcelain` output. With --line-porcelain
/// every line group repeats all keys, so a simple state machine
/// suffices: a 40-hex first token starts a group, "author"/
/// "author-time" keys fill it, and the tab-prefixed line is the
/// content that closes it.
pub fn parse_blame_porcelain(output: &str) -> Vec<BlameLine> {
    let mut lines = Vec::new();
    let mut sha = String::new();
    let mut author = String::new();
    let mut author_time = 0i64;
    for line in output.lines() {
        if let Some(text) = line.strip_prefix('\t') {
            lines.push(BlameLine {
                sha: std::mem::take(&mut sha),
                author: std::mem::take(&mut author),
                author_time,
                text: text.to_string(),
            });
        } else if let Some(rest) = line.strip_prefix("author ") {
            author = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("author-time ") {
            author_time = rest.trim().parse().unwrap_or(0);
        } else if line.len() > 40
            && line[..40].chars().all(|c| c.is_ascii_hexdigit())
            && line.as_bytes()[40] == b' '
        {
            sha = line[..40].to_string();
        }
    }
    lines
}

/// Epoch seconds to a `YYYY-MM-DD` date, UTC (Howard Hinnant's
/// civil-from-days algorithm).
pub fn format_epoch_utc(epoch: i64) -> String {
    let days = epoch.div_euclid(86400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Render blame lines as aligned columns:
/// `abcdef12  Author Name  2026-10-18  | code`. Uncommitted lines
/// (all-zero SHA) show dashes.
pub fn format_blame_lines(lines: &[BlameLine]) -> String {
    let width = lines
        .iter()
        .map(|l| l.author.chars().count())
        .max()
        .unwrap_or(0)
        .min(24);
    let mut out = String::new();
    for line in lines {
        let short: String = line.sha.chars().take(8).collect();
        let short = if short.chars().all(|c| c == '0') {
            "--------".to_string()
        } else {
            short
        };
        let author: String = line.author.chars().take(24).collect();
        let date = format_epoch_utc(line.author_time);
        let pad = width.saturating_sub(author.chars().count());
        out.push_str(&format!(
            "{short}  {author}{}  {date}  | {}",
            " ".repeat(pad),
            line.text
        ));
        out.push('\n');
    }
    out
}

/// `git blame --line-porcelain -- <rel_path>`, parsed and rendered
/// for a scratch editor.
pub fn git_blame_lines(root: &Path, rel_path: &Path) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("blame")
        .arg("--line-porcelain")
        .arg("--")
        .arg(rel_path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("Could not run git: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git blame failed:\n{}", clip_stderr(&stderr)));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines = parse_blame_porcelain(&stdout);
    if lines.is_empty() {
        return Err("git blame produced no output (empty file?).".to_string());
    }
    Ok(format_blame_lines(&lines))
}

/// `git stash push` (optionally including untracked files). Toast
/// text comes from stash_result_message.
pub fn stash_push(root: &Path, include_untracked: bool) -> Result<String, String> {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C").arg(root).arg("stash").arg("push");
    if include_untracked {
        cmd.arg("--include-untracked");
    }
    let out = cmd
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("Could not run git: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    stash_result_message(out.status.success(), &stdout, &stderr)
}

/// `git stash pop`. Toast text comes from stash_pop_result_message.
pub fn stash_pop(root: &Path) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("stash")
        .arg("pop")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("Could not run git: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    stash_pop_result_message(out.status.success(), &stdout, &stderr)
}

/// `git show --format=fuller <sha>`, clipped for a scratch editor.
pub fn git_show(root: &Path, sha: &str) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("show")
        .arg("--format=fuller")
        .arg(sha)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("Could not run git: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if out.status.success() {
        Ok(clip_show_output(&stdout))
    } else {
        Err(format!("git show failed:\n{}", clip_stderr(&stderr)))
    }
}

/// `git log` rows for the history palette: (full SHA, display text).
/// Field separator is U+001F so subjects may contain any printable
/// character. Parsing lives in parse_git_log_line for tests.
pub fn git_log_lines(root: &Path) -> Result<Vec<(String, String)>, String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("log")
        .arg("--pretty=format:%H%x1f%h%x1f%s%x1f%an%x1f%ar")
        .arg("-n")
        .arg("200")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("Could not run git: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(format!("git log failed:\n{}", clip_stderr(&stderr)));
    }
    Ok(stdout.lines().filter_map(parse_git_log_line).collect())
}

fn parse_git_log_line(line: &str) -> Option<(String, String)> {
    let mut fields = line.split('\u{1f}');
    let full = fields.next()?;
    let short = fields.next()?;
    let subject = fields.next()?;
    let author = fields.next()?;
    let rel = fields.next()?;
    if full.is_empty() || short.is_empty() {
        return None;
    }
    Some((
        full.to_string(),
        format!("{short} {subject} ({author}, {rel})"),
    ))
}

/// `git show` output capped for an editor buffer: generous, but a
/// mass-rename commit should not load hundreds of MB into a doc.
pub fn clip_show_output(text: &str) -> String {
    const CAP: usize = 512 * 1024;
    let trimmed = text.trim_end();
    if trimmed.len() <= CAP {
        return trimmed.to_string();
    }
    let mut end = CAP;
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[output clipped]", &trimmed[..end])
}

/// Toast body for `git stash pop`: same stdout/stderr split, plus the
/// reminder that a conflicting pop keeps the stash entry.
pub fn stash_pop_result_message(
    success: bool,
    stdout: &str,
    stderr: &str,
) -> Result<String, String> {
    let stdout = stdout.trim();
    if success {
        if !stdout.is_empty() {
            return Ok(stdout.to_string());
        }
        return Ok("Stash popped.".to_string());
    }
    Err(format!(
        "git stash pop failed:\n{}\n\nThe stash entry is kept when a pop conflicts.",
        clip_stderr(stderr)
    ))
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

    fn init_git_repo(dir: &std::path::Path) {
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.name", "test"],
            vec!["config", "user.email", "test@example.com"],
        ] {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(&args)
                .env("GIT_TERMINAL_PROMPT", "0")
                .status()
                .unwrap();
            assert!(status.success());
        }
    }

    fn staged_names(dir: &std::path::Path) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["diff", "--cached", "--name-only"])
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    #[test]
    fn stage_paths_adds_and_resets() {
        let dir = tempfile::tempdir().unwrap();
        init_git_repo(dir.path());
        std::fs::write(dir.path().join("a.txt"), "v1").unwrap();
        let path = dir.path().join("a.txt");

        stage_paths(dir.path(), std::slice::from_ref(&path), true).unwrap();
        assert!(staged_names(dir.path()).contains("a.txt"));

        stage_paths(dir.path(), std::slice::from_ref(&path), false).unwrap();
        assert!(!staged_names(dir.path()).contains("a.txt"));
    }

    // Written as one long literal: Rust's backslash-newline
    // continuation strips leading whitespace, which would eat the
    // leading space that marks context lines in a real diff.
    const SAMPLE_DIFF: &str = "diff --git a/a.txt b/a.txt\nindex 1111111..2222222 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,4 @@\n one\n+two changed\n two\n three\n@@ -10 +11,2 @@\n ten\n+eleven changed\n eleven\n";

    #[test]
    fn split_file_diff_parses_header_and_hunks() {
        let (header, hunks) = split_file_diff(SAMPLE_DIFF).unwrap();
        assert!(header.starts_with("diff --git a/a.txt"));
        assert!(header.ends_with("+++ b/a.txt\n"));
        assert_eq!(hunks.len(), 2);
        assert_eq!((hunks[0].new_start, hunks[0].new_len), (1, 4));
        // Omitted length means 1 for the old side; the new side here
        // is "11,2".
        assert_eq!((hunks[1].new_start, hunks[1].new_len), (11, 2));
        assert!(hunks[0].text.starts_with("@@ -1,3 +1,4 @@\n"));
        assert!(hunks[0].text.ends_with(" three\n"));
    }

    #[test]
    fn hunk_at_line_boundaries() {
        let (_, hunks) = split_file_diff(SAMPLE_DIFF).unwrap();
        // First hunk covers new lines 1..=4.
        assert_eq!(hunk_at_line(&hunks, 1), Some(0));
        assert_eq!(hunk_at_line(&hunks, 4), Some(0));
        assert_eq!(hunk_at_line(&hunks, 5), None);
        // Second hunk covers new lines 11..=12.
        assert_eq!(hunk_at_line(&hunks, 10), None);
        assert_eq!(hunk_at_line(&hunks, 11), Some(1));
        assert_eq!(hunk_at_line(&hunks, 12), Some(1));
        assert_eq!(hunk_at_line(&hunks, 13), None);
    }

    #[test]
    fn hunk_at_line_pure_deletion_matches_adjacent_lines() {
        let diff = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -3,2 +2,0 @@\n-gone one\n-gone two\n";
        let (_, hunks) = split_file_diff(diff).unwrap();
        assert_eq!((hunks[0].new_start, hunks[0].new_len), (2, 0));
        assert_eq!(hunk_at_line(&hunks, 2), Some(0));
        assert_eq!(hunk_at_line(&hunks, 3), Some(0));
        assert_eq!(hunk_at_line(&hunks, 4), None);
    }

    #[test]
    fn build_single_hunk_patch_keeps_file_header() {
        let (header, hunks) = split_file_diff(SAMPLE_DIFF).unwrap();
        let patch = build_single_hunk_patch(&header, &hunks[1]);
        let expected = "diff --git a/a.txt b/a.txt\nindex 1111111..2222222 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -10 +11,2 @@\n ten\n+eleven changed\n eleven\n";
        assert_eq!(patch, expected);
    }

    #[test]
    fn stage_hunk_at_line_stages_only_the_cursor_hunk() {
        let dir = tempfile::tempdir().unwrap();
        init_git_repo(dir.path());
        let path = dir.path().join("a.txt");
        let mut content = String::new();
        for i in 1..=40 {
            content.push_str(&format!("line {i}\n"));
        }
        std::fs::write(&path, &content).unwrap();
        stage_paths(dir.path(), std::slice::from_ref(&path), true).unwrap();

        // Two distant workdir edits; stage only the second one.
        let content = content
            .replacen("line 3\n", "line 3 changed\n", 1)
            .replacen("line 38\n", "line 38 changed\n", 1);
        std::fs::write(&path, &content).unwrap();

        let staged = stage_hunk_at_line(
            dir.path(),
            std::path::Path::new("a.txt"),
            38,
        )
        .unwrap();
        assert!(staged);

        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["diff", "--cached"])
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .unwrap();
        let cached = String::from_utf8_lossy(&out.stdout);
        assert!(cached.contains("line 38 changed"));
        assert!(!cached.contains("line 3 changed"));

        // A line with no hunk reports Ok(false), not an error.
        let dir2 = tempfile::tempdir().unwrap();
        init_git_repo(dir2.path());
        std::fs::write(dir2.path().join("b.txt"), "one\n").unwrap();
        stage_paths(dir2.path(), &[dir2.path().join("b.txt")], true).unwrap();
        assert!(!stage_hunk_at_line(
            dir2.path(),
            std::path::Path::new("b.txt"),
            1
        )
        .unwrap());
    }

    #[test]
    fn format_epoch_utc_golden() {
        assert_eq!(format_epoch_utc(0), "1970-01-01");
        assert_eq!(format_epoch_utc(86400), "1970-01-02");
        assert_eq!(format_epoch_utc(1_750_000_000), "2025-06-15");
        // Negative epochs still produce sane dates.
        assert_eq!(format_epoch_utc(-86400), "1969-12-31");
    }

    #[test]
    fn parse_blame_porcelain_groups() {
        let sample = "abc123000000000000000000000000000000abcd 1 1 1\n\
author Alice Example\n\
author-time 1750000000\n\
summary first line\n\
\tone\n\
0000000000000000000000000000000000000000 2 2 1\n\
author Not Committed Yet\n\
author-time 1750000001\n\
summary dirty\n\
\ttwo\n";
        let lines = parse_blame_porcelain(sample);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].author, "Alice Example");
        assert_eq!(lines[0].author_time, 1_750_000_000);
        assert_eq!(lines[0].text, "one");
        assert_eq!(lines[1].author, "Not Committed Yet");
    }

    #[test]
    fn format_blame_lines_aligns_columns() {
        // Exactly 40 hex chars: the parser keys on hex[40] + space.
        let sha = format!("abc12300{}", "0".repeat(32));
        let sample = format!("{sha} 1 1 1\n\
author Alice Example\n\
author-time 1750000000\n\
\tone\n\
0000000000000000000000000000000000000000 2 2 1\n\
author Not Committed Yet\n\
author-time 1750000001\n\
\ttwo\n");
        let lines = parse_blame_porcelain(&sample);
        let out = format_blame_lines(&lines);
        assert_eq!(
            out,
            "abc12300  Alice Example      2025-06-15  | one\n--------  Not Committed Yet  2025-06-15  | two\n"
        );
    }

    #[test]
    fn git_blame_lines_real_repo() {
        let dir = tempfile::tempdir().unwrap();
        init_git_repo(dir.path());
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").unwrap();
        stage_paths(dir.path(), &[dir.path().join("a.txt")], true).unwrap();
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["commit", "-q", "-m", "init"])
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .unwrap();
        assert!(status.success());

        let blame =
            git_blame_lines(dir.path(), std::path::Path::new("a.txt")).unwrap();
        assert!(blame.contains("test"));
        assert!(blame.contains("| one"));
        assert!(blame.contains("| two"));
    }

    fn commit_file(dir: &std::path::Path, name: &str, content: &str, message: &str) {
        std::fs::write(dir.join(name), content).unwrap();
        stage_paths(dir, &[dir.join(name)], true).unwrap();
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["commit", "-q", "-m", message])
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn status_porcelain(dir: &std::path::Path) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["status", "--porcelain"])
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    #[test]
    fn stash_push_pop_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        init_git_repo(dir.path());
        commit_file(dir.path(), "a.txt", "v1\n", "init");

        // Tracked modification stashes and pops back.
        std::fs::write(dir.path().join("a.txt"), "v2\n").unwrap();
        let message = stash_push(dir.path(), false).unwrap();
        assert!(message.contains("Saved"), "unexpected: {message}");
        assert_eq!(status_porcelain(dir.path()), "");
        stash_pop(dir.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "v2\n"
        );

        // Untracked files are left alone without the flag...
        std::fs::write(dir.path().join("b.txt"), "new\n").unwrap();
        stash_push(dir.path(), false).unwrap();
        assert!(dir.path().join("b.txt").exists());
        stash_pop(dir.path()).unwrap();

        // ...and stashed with it.
        stash_push(dir.path(), true).unwrap();
        assert!(!dir.path().join("b.txt").exists());
        stash_pop(dir.path()).unwrap();
        assert!(dir.path().join("b.txt").exists());

        // Nothing to stash reports cleanly, not as an error.
        std::fs::remove_file(dir.path().join("b.txt")).unwrap();
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["checkout", "--", "a.txt"])
            .status()
            .unwrap();
        assert!(status.success());
        let message = stash_push(dir.path(), false).unwrap();
        assert!(message.contains("No local changes"), "unexpected: {message}");
    }

    #[test]
    fn git_log_lines_real_repo() {
        let dir = tempfile::tempdir().unwrap();
        init_git_repo(dir.path());
        commit_file(dir.path(), "a.txt", "v1\n", "first commit");
        commit_file(dir.path(), "b.txt", "v1\n", "second commit");

        let rows = git_log_lines(dir.path()).unwrap();
        assert_eq!(rows.len(), 2);
        // Newest first; full 40-char SHA; subject and author present.
        assert_eq!(rows[0].0.len(), 40);
        assert!(rows[0].1.contains("second commit"));
        assert!(rows[0].1.contains("test"));
        assert!(rows[1].1.contains("first commit"));
    }

    #[test]
    fn git_show_real_commit_and_bad_sha() {
        let dir = tempfile::tempdir().unwrap();
        init_git_repo(dir.path());
        commit_file(dir.path(), "a.txt", "hello\n", "add greeting");

        let rows = git_log_lines(dir.path()).unwrap();
        let text = git_show(dir.path(), &rows[0].0).unwrap();
        assert!(text.contains("add greeting"));
        assert!(text.contains("+++ b/a.txt"));

        assert!(git_show(dir.path(), "deadbeefdeadbeef").is_err());
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

    #[test]
    fn stash_message_prefers_stdout_on_success() {
        let msg = stash_result_message(
            true,
            "Saved working directory and index state WIP on main: abc123\n",
            "",
        )
        .unwrap();
        assert!(msg.starts_with("Saved working directory"));
        let msg = stash_pop_result_message(true, "Dropped refs/stash@{0}", "")
            .unwrap();
        assert_eq!(msg, "Dropped refs/stash@{0}");
    }

    #[test]
    fn stash_message_empty_and_failure_cases() {
        assert_eq!(stash_result_message(true, "", "").unwrap(), "Stashed.");
        assert_eq!(
            stash_pop_result_message(true, " \n", "").unwrap(),
            "Stash popped."
        );
        let err = stash_result_message(false, "", "error: nope").unwrap_err();
        assert!(err.starts_with("git stash failed:"));
        assert!(err.contains("error: nope"));
        let err =
            stash_pop_result_message(false, "", "No stash entries found.")
                .unwrap_err();
        assert!(err.contains("No stash entries found."));
        assert!(err.contains("kept when a pop conflicts"));
    }

    #[test]
    fn merge_abort_message_cases() {
        assert_eq!(
            merge_abort_result_message(true, "", "").unwrap(),
            "Merge aborted."
        );
        let err = merge_abort_result_message(
            false,
            "",
            "fatal: There is no merge in progress",
        )
        .unwrap_err();
        assert!(err.contains("no merge in progress"));
    }

    #[test]
    fn git_log_line_parses_fields() {
        let (sha, display) = parse_git_log_line(
            "abc123def\x1fabc123d\x1ffix: modal Esc chain\x1fStefan\x1f2 hours ago",
        )
        .unwrap();
        assert_eq!(sha, "abc123def");
        assert_eq!(
            display,
            "abc123d fix: modal Esc chain (Stefan, 2 hours ago)"
        );
        assert!(parse_git_log_line("no separators").is_none());
        assert!(parse_git_log_line("\x1f\x1fs\x1fa\x1fr").is_none());
    }

    #[test]
    fn clip_show_output_respects_cap_and_boundaries() {
        assert_eq!(clip_show_output("short\n"), "short");
        let big = "\u{65e5}".repeat(200_000); // 600 KB, 3-byte chars
        let out = clip_show_output(&big);
        assert!(out.ends_with("[output clipped]"));
        let clipped_len = out.len() - "\n[output clipped]".len();
        assert!(clipped_len <= 512 * 1024);
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
    }
}
