//! Project-configured external formatters (Prettier, Laravel Pint, php-cs-fixer)
//! for format-on-save.
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
/// PHP tools boot the framework autoloader and write a file, so they get longer.
const PHP_TIMEOUT: Duration = Duration::from_millis(4000);

const PINT_CONFIG: &str = "pint.json";
const CS_FIXER_CONFIGS: &[&str] = &[".php-cs-fixer.php", ".php-cs-fixer.dist.php"];
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
pub enum Kind {
    Prettier,
    /// Laravel Pint, run on a temporary copy of the buffer.
    Pint,
    /// php-cs-fixer with the project's config, run on a temporary copy.
    PhpCsFixer { config: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formatter {
    pub program: PathBuf,
    pub root: PathBuf,
    pub kind: Kind,
}

/// Pint if the project has `vendor/bin/pint`; otherwise php-cs-fixer when a
/// `.php-cs-fixer[.dist].php` config and `vendor/bin/php-cs-fixer` exist.
fn resolve_php(file: &Path) -> Option<Formatter> {
    let mut dir = file.parent();
    while let Some(d) = dir {
        let bin = d.join("vendor").join("bin");
        if d.join("composer.json").is_file() {
            let pint = bin.join("pint");
            if pint.is_file() {
                return Some(Formatter { program: pint, root: d.to_path_buf(), kind: Kind::Pint });
            }
            let fixer = bin.join("php-cs-fixer");
            if fixer.is_file() {
                if let Some(config) = CS_FIXER_CONFIGS.iter().map(|n| d.join(n)).find(|p| p.is_file()) {
                    return Some(Formatter {
                        program: fixer,
                        root: d.to_path_buf(),
                        kind: Kind::PhpCsFixer { config },
                    });
                }
            }
            return None;
        }
        dir = d.parent();
    }
    None
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
    if ext == "php" {
        return resolve_php(file);
    }
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
                kind: Kind::Prettier,
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
    match &formatter.kind {
        Kind::Prettier => format_stdin(formatter, file, text),
        Kind::Pint | Kind::PhpCsFixer { .. } => format_temp_file(formatter, text),
    }
}

/// Pint and php-cs-fixer only format files, so the buffer goes to a temporary
/// `.php` file that is formatted in place and read back. The real file is never
/// touched. Pint runs with the project's `pint.json` if present.
fn format_temp_file(formatter: &Formatter, text: &str) -> Option<String> {
    let dir = tempfile::tempdir().ok()?;
    let tmp = dir.path().join("buffer.php");
    std::fs::write(&tmp, text).ok()?;
    let mut cmd = Command::new(&formatter.program);
    match &formatter.kind {
        Kind::Pint => {
            let cfg = formatter.root.join(PINT_CONFIG);
            cmd.arg(&tmp);
            if cfg.is_file() {
                cmd.arg("--config").arg(cfg);
            }
        }
        Kind::PhpCsFixer { config } => {
            cmd.arg("fix")
                .arg("--config")
                .arg(config)
                .arg("--using-cache=no")
                .arg("--no-interaction")
                .arg("--quiet")
                .arg(&tmp);
        }
        Kind::Prettier => return None,
    }
    let ok = run_with_timeout(
        cmd.current_dir(&formatter.root).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()),
        None,
        PHP_TIMEOUT,
    )
    .is_some_and(|(status, _)| status);
    if !ok {
        return None;
    }
    let meta = std::fs::metadata(&tmp).ok()?;
    if meta.len() as usize > MAX_BYTES {
        return None;
    }
    String::from_utf8(std::fs::read(&tmp).ok()?).ok()
}

fn format_stdin(formatter: &Formatter, file: &Path, text: &str) -> Option<String> {
    let mut cmd = Command::new(&formatter.program);
    cmd.arg("--stdin-filepath")
        .arg(file)
        .current_dir(&formatter.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let (ok, out) = run_with_timeout(&mut cmd, Some(text), TIMEOUT)?;
    if !ok || out.len() > MAX_BYTES {
        return None;
    }
    String::from_utf8(out).ok()
}

/// Runs `cmd`, feeding `input` on stdin when given, killing it after `limit`.
/// Returns (success, stdout).
fn run_with_timeout(
    cmd: &mut Command,
    input: Option<&str>,
    limit: Duration,
) -> Option<(bool, Vec<u8>)> {
    let mut child = cmd

        .spawn()
        .ok()?;
    let writer = match (input, child.stdin.take()) {
        (Some(text), Some(mut stdin)) => {
            let input = text.to_string();
            Some(std::thread::spawn(move || {
                let _ = stdin.write_all(input.as_bytes());
            }))
        }
        _ => None,
    };
    let reader = child.stdout.take().map(|mut stdout| {
        std::thread::spawn(move || {
            let mut out = Vec::new();
            let _ = stdout.by_ref().take(MAX_BYTES as u64 + 1).read_to_end(&mut out);
            out
        })
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if start.elapsed() < limit => {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    if let Some(w) = writer {
        let _ = w.join();
    }
    let out = reader.map(|r| r.join().unwrap_or_default()).unwrap_or_default();
    Some((status?.success(), out))
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
        let f = Formatter { program: fake, root: dir.path().to_path_buf(), kind: Kind::Prettier };
        assert_eq!(format_retry(&f, &dir.path().join("a.js"), "ab\n").as_deref(), Some("AB\n"));
        let bad = Formatter { program: dir.path().join("missing"), root: dir.path().to_path_buf(), kind: Kind::Prettier };
        assert!(format(&bad, &dir.path().join("a.js"), "x").is_none());
        let fail = dir.path().join("fail");
        std::fs::write(&fail, "#!/bin/sh\nexit 2\n").unwrap();
        std::fs::set_permissions(&fail, std::fs::Permissions::from_mode(0o755)).unwrap();
        let f = Formatter { program: fail, root: dir.path().to_path_buf(), kind: Kind::Prettier };
        assert!(format(&f, &dir.path().join("a.js"), "x").is_none());
    }

    #[test]
    fn unchanged_text_gives_no_edit() {
        assert!(whole_document_edit("a", "a".into(), Position::new(0, 1)).is_empty());
        assert_eq!(whole_document_edit("a", "b".into(), Position::new(0, 1)).len(), 1);
    }

    /// `format` for stand-in scripts written a moment ago. Another test thread
    /// forking while our write handle is still open makes `exec` fail with
    /// ETXTBSY ("text file busy"), which `format` reports as `None`; a few
    /// short retries ride that out. Only use it for calls expected to succeed.
    #[cfg(unix)]
    fn format_retry(f: &Formatter, file: &Path, text: &str) -> Option<String> {
        for _ in 0..10 {
            if let Some(out) = format(f, file, text) {
                return Some(out);
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        None
    }

    fn touch_exec(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn php_resolves_pint_then_fixer_and_needs_composer_project() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("app/Models/User.php");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        // Not a composer project: no formatter.
        touch_exec(&dir.path().join("vendor/bin/pint"), "");
        assert!(resolve(&f).is_none());
        std::fs::write(dir.path().join("composer.json"), "{}").unwrap();
        assert_eq!(resolve(&f).unwrap().kind, Kind::Pint);
        std::fs::remove_file(dir.path().join("vendor/bin/pint")).unwrap();
        // Fixer binary without config is not an opt-in.
        touch_exec(&dir.path().join("vendor/bin/php-cs-fixer"), "");
        assert!(resolve(&f).is_none());
        std::fs::write(dir.path().join(".php-cs-fixer.dist.php"), "<?php").unwrap();
        assert!(matches!(resolve(&f).unwrap().kind, Kind::PhpCsFixer { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn pint_runs_on_a_temp_copy_and_leaves_the_real_file_alone() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("composer.json"), "{}").unwrap();
        std::fs::write(dir.path().join("pint.json"), "{}").unwrap();
        let real = dir.path().join("a.php");
        std::fs::write(&real, "<?php //real").unwrap();
        // Stand-in Pint: upper-cases the file named by $1 in place, checks --config.
        let pint = dir.path().join("vendor/bin/pint");
        touch_exec(&pint, "#!/bin/sh\n[ \"$2\" = \"--config\" ] || exit 3\ntr a-z A-Z < \"$1\" > \"$1.x\" && mv \"$1.x\" \"$1\"\n");
        let f = Formatter { program: pint, root: dir.path().to_path_buf(), kind: Kind::Pint };
        assert_eq!(format_retry(&f, &real, "<?php echo 1;\n").as_deref(), Some("<?PHP ECHO 1;\n"));
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "<?php //real");
    }

    #[cfg(unix)]
    #[test]
    fn php_cs_fixer_gets_config_and_failures_are_quiet() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join(".php-cs-fixer.php");
        std::fs::write(&cfg, "<?php").unwrap();
        // Arguments: fix --config <cfg> --using-cache=no --no-interaction --quiet <file>
        let fixer = dir.path().join("fixer");
        touch_exec(&fixer, &format!("#!/bin/sh\n[ \"$1\" = fix ] && [ \"$3\" = \"{}\" ] || exit 3\nfor last; do :; done\nprintf 'fixed' > \"$last\"\n", cfg.display()));
        let f = Formatter { program: fixer.clone(), root: dir.path().to_path_buf(), kind: Kind::PhpCsFixer { config: cfg.clone() } };
        assert_eq!(format_retry(&f, &dir.path().join("a.php"), "x").as_deref(), Some("fixed"));
        let bad = Formatter { program: dir.path().join("missing"), root: dir.path().to_path_buf(), kind: Kind::PhpCsFixer { config: cfg } };
        assert!(format(&bad, &dir.path().join("a.php"), "x").is_none());
    }
}
