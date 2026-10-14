//! Linux-only bounded reads/search. Roots are pinned handles; paths never become authority.
//! Honors repository .gitignore only, not user global Git excludes. No execution side effects.
#[cfg(target_os = "linux")]
mod linux {
    use ignore::gitignore::{Gitignore, GitignoreBuilder};
    use rustix::fs::{Dir, Mode, OFlags, ResolveFlags, openat2};
    use std::{
        fs::File,
        io::Read,
        path::{Component, Path, PathBuf},
        time::{Duration, Instant},
    };
    const FILE_LIMIT: usize = 256 * 1024;
    pub struct ReadRoot {
        root: File,
    }
    #[derive(Debug)]
    pub struct SearchResult {
        pub matches: Vec<(String, usize, String)>,
        pub truncated: bool,
        pub skipped_files: usize,
    }
    fn allowed(path: &Path) -> bool {
        !path.as_os_str().is_empty()
            && path.components().all(|c| match c {
                Component::Normal(n) => n.to_str().is_some_and(|s| {
                    let s = s.to_ascii_lowercase();
                    !s.starts_with('.')
                        && !matches!(
                            s.as_str(),
                            "vendor"
                                | "node_modules"
                                | "credentials"
                                | "secrets"
                                | "id_rsa"
                                | "id_ed25519"
                        )
                        && ![".pem", ".key", ".p12", ".pfx"]
                            .iter()
                            .any(|ext| s.ends_with(ext))
                }),
                _ => false,
            })
    }
    impl ReadRoot {
        pub fn new(owner_root: &Path, trusted: bool) -> Result<Self, String> {
            if !trusted {
                return Err("Explicit workspace trust required".into());
            }
            let canonical = owner_root.canonicalize().map_err(|e| e.to_string())?;
            // Open the selected root without following a concurrently substituted symlink.
            let cwd = File::open("/").map_err(|e| e.to_string())?;
            let relative = canonical
                .strip_prefix("/")
                .map_err(|_| "Invalid absolute root")?;
            let root = File::from(
                openat2(
                    &cwd,
                    relative,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                    Mode::empty(),
                    ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
                )
                .map_err(|e| format!("Secure root open denied: {e}"))?,
            );
            if !root.metadata().map_err(|e| e.to_string())?.is_dir() {
                return Err("Root must be a directory".into());
            }
            // Probe enforced resolution. Unsupported kernels fail closed, never std::fs fallback.
            openat2(
                &root,
                ".",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::BENEATH
                    | ResolveFlags::NO_SYMLINKS
                    | ResolveFlags::NO_XDEV,
            )
            .map_err(|e| format!("Secure file access unavailable: {e}"))?;
            Ok(Self { root })
        }
        fn open(&self, path: &Path, directory: bool) -> Result<File, String> {
            let mut flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK;
            if directory {
                flags |= OFlags::DIRECTORY;
            }
            let fd = openat2(
                &self.root,
                path,
                flags,
                Mode::empty(),
                ResolveFlags::BENEATH
                    | ResolveFlags::NO_SYMLINKS
                    | ResolveFlags::NO_XDEV,
            )
            .map_err(|e| format!("Secure access denied: {e}"))?;
            Ok(File::from(fd))
        }
        fn text(&self, path: &Path, max: usize) -> Result<String, String> {
            let f = self.open(path, false)?;
            let m = f.metadata().map_err(|e| e.to_string())?;
            if !m.is_file() || m.len() > max as u64 {
                return Err("Not a bounded regular text file".into());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if m.nlink() > 1 {
                    return Err("Hard-linked files denied".into());
                }
            }
            let mut bytes = Vec::new();
            f.take(max as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > max || bytes.contains(&0) {
                return Err("Oversized or binary file".into());
            }
            String::from_utf8(bytes).map_err(|_| "Non-UTF8 file".into())
        }
        fn ignores(&self, parent: &Path) -> Result<Gitignore, String> {
            let mut builder = GitignoreBuilder::new(".");
            let mut directories = vec![PathBuf::new()];
            let mut current = PathBuf::new();
            for c in parent.components() {
                current.push(c.as_os_str());
                directories.push(current.clone());
            }
            for dir in directories {
                let p = dir.join(".gitignore");
                match self.open(&p, false) {
                    Ok(f) => {
                        let mut bytes = Vec::new();
                        let meta = f.metadata().map_err(|e| e.to_string())?;
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::MetadataExt;
                            if meta.nlink() > 1 {
                                return Err("Hard-linked gitignore denied".into());
                            }
                        }
                        if !meta.is_file() || meta.len() > 65536 {
                            return Err("Invalid gitignore file".into());
                        }
                        f.take(65537)
                            .read_to_end(&mut bytes)
                            .map_err(|e| e.to_string())?;
                        if bytes.len() > 65536 {
                            return Err("Gitignore too large".into());
                        }
                        let text = String::from_utf8(bytes)
                            .map_err(|_| "Gitignore is not UTF8")?;
                        for line in text.lines() {
                            builder
                                .add_line(Some(p.clone()), line)
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    Err(_) => {
                        // Distinguish absence through secure open errno, not pathname metadata.
                        match openat2(
                            &self.root,
                            &p,
                            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
                            Mode::empty(),
                            ResolveFlags::BENEATH
                                | ResolveFlags::NO_SYMLINKS
                                | ResolveFlags::NO_XDEV,
                        ) {
                            Err(rustix::io::Errno::NOENT) => {}
                            _ => {
                                return Err(
                                    "Gitignore could not be safely read".into()
                                );
                            }
                        }
                    }
                }
            }
            builder.build().map_err(|e| e.to_string())
        }
        pub fn read(&self, path: &Path) -> Result<String, String> {
            if !allowed(path) {
                return Err("Sensitive, dot or traversing path denied".into());
            }
            if self
                .ignores(path.parent().unwrap_or(Path::new("")))?
                .matched_path_or_any_parents(path, false)
                .is_ignore()
            {
                return Err("Gitignored file denied".into());
            }
            self.text(path, FILE_LIMIT)
        }
        pub fn search(&self, needle: &str) -> Result<SearchResult, String> {
            if needle.is_empty() || needle.len() > 256 || needle.contains('\n') {
                return Err(
                    "Search requires a nonempty single-line literal up to 256 bytes"
                        .into(),
                );
            }
            let start = Instant::now();
            let mut queue = vec![PathBuf::from(".")];
            let mut entries = 0;
            let mut total = 0;
            let mut result = SearchResult {
                matches: Vec::new(),
                truncated: false,
                skipped_files: 0,
            };
            while let Some(dir) = queue.pop() {
                if start.elapsed() > Duration::from_secs(2) {
                    result.truncated = true;
                    break;
                }
                let folder = self.open(&dir, true)?;
                let ignores = self.ignores(if dir == Path::new(".") {
                    Path::new("")
                } else {
                    &dir
                })?;
                let listing = Dir::read_from(&folder).map_err(|e| e.to_string())?;
                for item in listing {
                    entries += 1;
                    if entries > 5000
                        || total > 8 * 1024 * 1024
                        || result.matches.len() >= 100
                        || start.elapsed() > Duration::from_secs(2)
                    {
                        result.truncated = true;
                        return Ok(result);
                    }
                    let item = item.map_err(|e| e.to_string())?;
                    let name = item
                        .file_name()
                        .to_str()
                        .map_err(|_| "Non-UTF8 directory entry")?;
                    if name == "." || name == ".." {
                        continue;
                    }
                    let path = if dir == Path::new(".") {
                        PathBuf::from(name)
                    } else {
                        dir.join(name)
                    };
                    if !allowed(&path) {
                        continue;
                    }
                    let is_dir = item.file_type() == rustix::fs::FileType::Directory;
                    if ignores
                        .matched_path_or_any_parents(&path, is_dir)
                        .is_ignore()
                    {
                        continue;
                    }
                    if is_dir {
                        if path.components().count() <= 8 {
                            queue.push(path);
                        } else {
                            result.truncated = true;
                        }
                        continue;
                    }
                    match self.read(&path) {
                        Ok(text) => {
                            total += text.len();
                            for (i, line) in text.lines().enumerate() {
                                if line.contains(needle) {
                                    let mut end = line.len().min(1024);
                                    while !line.is_char_boundary(end) {
                                        end -= 1;
                                    }
                                    result.matches.push((
                                        path.to_string_lossy().into_owned(),
                                        i + 1,
                                        line[..end].into(),
                                    ));
                                    if result.matches.len() >= 100 {
                                        result.truncated = true;
                                        return Ok(result);
                                    }
                                }
                            }
                        }
                        Err(_) => result.skipped_files += 1,
                    }
                }
            }
            Ok(result)
        }
    }
}
#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::{fs, path::Path};
    #[test]
    fn read_and_literal_search_respect_ignores() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join(".gitignore"), "hidden.php\nsub/*.tmp\n").unwrap();
        fs::write(d.path().join("visible.php"), "hello 世界\nneedle\n").unwrap();
        fs::write(d.path().join("hidden.php"), "needle secret").unwrap();
        fs::create_dir(d.path().join("sub")).unwrap();
        fs::write(d.path().join("sub/a.tmp"), "needle").unwrap();
        let r = ReadRoot::new(d.path(), true).unwrap();
        assert!(r.read(Path::new("hidden.php")).is_err());
        let matches = r.search("needle").unwrap();
        assert_eq!(matches.matches.len(), 1);
        assert_eq!(matches.matches[0].0, "visible.php");
        assert_eq!(matches.matches[0].1, 2);
    }
    #[test]
    fn symlink_hardlink_special_and_secret_denied() {
        let d = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret"), "outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), d.path().join("escape")).unwrap();
        fs::hard_link(outside.path().join("secret"), d.path().join("hard")).unwrap();
        fs::write(d.path().join(".env"), "secret").unwrap();
        let r = ReadRoot::new(d.path(), true).unwrap();
        for p in ["escape/secret", "hard", ".env", "../secret", "/etc/passwd"] {
            assert!(r.read(Path::new(p)).is_err());
        }
        assert!(ReadRoot::new(d.path(), false).is_err());
    }
    #[test]
    fn nested_gitignore_and_pinned_root() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("project")).unwrap();
        fs::create_dir(d.path().join("project/sub")).unwrap();
        fs::write(d.path().join("project/sub/.gitignore"), "secret.txt\n").unwrap();
        fs::write(d.path().join("project/sub/secret.txt"), "secret").unwrap();
        let r = ReadRoot::new(&d.path().join("project"), true).unwrap();
        fs::rename(d.path().join("project"), d.path().join("old")).unwrap();
        fs::create_dir(d.path().join("project")).unwrap();
        fs::write(d.path().join("project/replacement.txt"), "outside").unwrap();
        assert!(r.read(Path::new("replacement.txt")).is_err());
        assert!(r.read(Path::new("sub/secret.txt")).is_err());
    }
    #[test]
    fn binary_oversize_bad_ignore_and_bounds() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("binary"), [0u8, 1]).unwrap();
        fs::write(d.path().join("big"), vec![b'x'; 300000]).unwrap();
        let r = ReadRoot::new(d.path(), true).unwrap();
        assert!(r.read(Path::new("binary")).is_err());
        assert!(r.read(Path::new("big")).is_err());
        assert!(r.search("").is_err());
        fs::write(d.path().join("many"), "needle\n".repeat(101)).unwrap();
        let result = r.search("needle").unwrap();
        assert_eq!(result.matches.len(), 100);
        assert!(result.truncated);
    }
    #[test]
    fn swapped_symlink_and_invalid_gitignore_fail_closed() {
        let d = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("sub")).unwrap();
        fs::write(d.path().join("sub/file"), "inside").unwrap();
        fs::write(outside.path().join("file"), "outside").unwrap();
        let root = ReadRoot::new(d.path(), true).unwrap();
        fs::rename(d.path().join("sub"), d.path().join("old")).unwrap();
        std::os::unix::fs::symlink(outside.path(), d.path().join("sub")).unwrap();
        assert!(root.read(Path::new("sub/file")).is_err());
        std::os::unix::fs::symlink(
            outside.path().join("file"),
            d.path().join(".gitignore"),
        )
        .unwrap();
        assert!(root.read(Path::new("old/file")).is_err());
        assert!(root.search("inside").is_err());
    }
    #[test]
    fn hardlinked_gitignore_is_not_trusted() {
        let d = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        fs::write(other.path().join("ignore"), "secret.txt\n").unwrap();
        fs::hard_link(other.path().join("ignore"), d.path().join(".gitignore"))
            .unwrap();
        fs::write(d.path().join("file.txt"), "needle").unwrap();
        let r = ReadRoot::new(d.path(), true).unwrap();
        assert!(r.read(Path::new("file.txt")).is_err());
    }
}
