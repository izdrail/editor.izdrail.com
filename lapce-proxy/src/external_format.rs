//! Project-configured external formatters (Prettier) for format-on-save.
//!
//! A formatter is used only when the project opts in: a Prettier config file
//! (or a `prettier` key in `package.json`) exists in the file's project and a
//! `prettier` binary is found (`node_modules/.bin` first, then `PATH`). The
//! buffer text goes to the formatter on stdin, so unsaved edits are formatted
//! and nothing is written to disk. Failures are quiet: no formatter, a
//! non-zero exit or a timeout simply mean no edits.
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use lsp_types::{Position, Range, TextEdit};

const TIMEOUT: Duration = Duration::from_millis(900);
const MAX_BYTES: usize = 4 * 1024 * 1024;

const CONFIG_NAMES: &[&str] = &[
    ".prettierrc",
    ".prettierrc.json",
    ".prettierrc.yml",
    ".prettierrc.yaml",
    ".prettierrc.json5",
    ".prettierrc.js",
    ".prettierrc.cjs",
    ".prettierrc.mjs",
    ".prettierrc.toml",
    "prettier.config.js",
    "prettier.config.cjs",
    "prettier.config.mjs",
    "prettier.config.ts",
];

const EXTENSIONS: &[&str] = &[
    "js", "jsx", "mjs", "cjs", "ts", "tsx", "json", "css", "scss", "less",
    "html", "vue", "md", "yaml", "yml",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formatter {
    pub program: PathBuf,
    pub root: PathBuf,
}

fn has_prettier_config(dir: &Path) -> bool {
    if CONFIG_NAMES.iter().any(|n| dir.join(n).is_file()) {
        return true;
    }
    let pkg = dir.join("package.json");
    match std::fs::metadata(&pkg) {
        Ok(m) if m.is_file() && m.len() <= 1024 * 1024 => {
            std::fs::read_to_string(&pkg)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .is_some_and(|v| v.get("prettier").is_some())
        }
        _ => false,
    }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// The formatter for `file`, if its project asks for one.
pub fn resolve(file: &Path) -> Option<Formatter> {
    let ext = file.extension()?.to_str()?;
    if !EXTENSIONS.contains(&ext) {
        return None;
    }
    let mut dir = file.parent();
    while let Some(d) = dir {
        if has_prettier_config(d) {
            let local = d
                .ancestors()
                .map(|a| a.join("node_modules").join(".bin").join("prettier"))
                .find(|p| p.is_file());
            let program = local.or_else(|| find_on_path("prettier"))?;
            return Some(Formatter {
                program,
                root: d.to_path_buf(),
            });
        }
        dir = d.parent();
    }
    None
}

/// Formats `text` as the content of `file`. `None` on any failure.
pub fn format(formatter: &Formatter, file: &Path, text: &str) -> Option<String> {
    if text.len() > MAX_BYTES {
        return None;
    }
    let mut child = Command::new(&formatter.program)
        .arg("--stdin-filepath")
        .arg(file)
        .current_dir(&formatter.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    let input = text.to_string();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = stdout.by_ref().take(MAX_BYTES as u64 + 1).read_to_end(&mut out);
        out
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if start.elapsed() < TIMEOUT => {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let _ = writer.join();
    let out = reader.join().ok()?;
    if !status?.success() || out.len() > MAX_BYTES {
        return None;
    }
    String::from_utf8(out).ok()
}

/// One edit replacing the whole document, or none when nothing changed.
/// `end` is the position of the end of the old text.
pub fn whole_document_edit(old: &str, new: String, end: Position) -> Vec<TextEdit> {
    if old == new {
        return Vec::new();
    }
    vec![TextEdit {
        range: Range {
            start: Position::new(0, 0),
            end,
        },
        new_text: new,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_config_means_no_formatter() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.js");
        std::fs::write(&file, "x").unwrap();
        assert!(resolve(&file).is_none());
    }

    #[test]
    fn unsupported_extension_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".prettierrc"), "{}").unwrap();
        assert!(resolve(&dir.path().join("a.php")).is_none());
    }

    #[test]
    fn config_plus_local_binary_resolves() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".prettierrc"), "{}").unwrap();
        let bin = dir.path().join("node_modules/.bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("prettier"), "").unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir_all(&sub).unwrap();
        let f = resolve(&sub.join("a.ts")).unwrap();
        assert_eq!(f.root, dir.path());
        assert!(f.program.ends_with("node_modules/.bin/prettier"));
    }

    #[test]
    fn package_json_prettier_key_counts_as_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"prettier":{}}"#).unwrap();
        let bin = dir.path().join("node_modules/.bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("prettier"), "").unwrap();
        assert!(resolve(&dir.path().join("a.css")).is_some());
    }

    #[cfg(unix)]
    #[test]
    fn formats_through_stdin_and_fails_quietly() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake");
        // Stand-in formatter: upper-cases stdin.
        std::fs::write(&fake, "#!/bin/sh\ntr a-z A-Z\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let f = Formatter { program: fake, root: dir.path().to_path_buf() };
        assert_eq!(format(&f, &dir.path().join("a.js"), "ab\n").as_deref(), Some("AB\n"));
        let bad = Formatter { program: dir.path().join("missing"), root: dir.path().to_path_buf() };
        assert!(format(&bad, &dir.path().join("a.js"), "x").is_none());
        let fail = dir.path().join("fail");
        std::fs::write(&fail, "#!/bin/sh\nexit 2\n").unwrap();
        std::fs::set_permissions(&fail, std::fs::Permissions::from_mode(0o755)).unwrap();
        let f = Formatter { program: fail, root: dir.path().to_path_buf() };
        assert!(format(&f, &dir.path().join("a.js"), "x").is_none());
    }

    #[test]
    fn unchanged_text_gives_no_edit() {
        assert!(whole_document_edit("a", "a".into(), Position::new(0, 1)).is_empty());
        assert_eq!(whole_document_edit("a", "b".into(), Position::new(0, 1)).len(), 1);
    }
}
