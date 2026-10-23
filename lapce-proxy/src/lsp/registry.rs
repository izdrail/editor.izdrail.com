//! Global-only server configuration. Project files cannot introduce install commands.
use anyhow::{Context, Result, ensure};
use lapce_core::directory::Directory;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf, process::Command};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Server {
    pub languages: Vec<String>,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
    /// Only offer this server inside a detected Laravel project: artisan at
    /// the nearest project root or a composer.json requiring
    /// laravel/framework. Gated-out servers fall through to the next enabled
    /// entry for the language.
    pub requires_laravel: bool,
    pub initialization_options: Value,
    pub packages: Vec<String>,
}
impl Default for Server {
    fn default() -> Self {
        Self {
            languages: vec![],
            command: String::new(),
            args: vec![],
            enabled: true,
            requires_laravel: false,
            initialization_options: serde_json::json!({}),
            packages: vec![],
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Registry {
    pub enabled: bool,
    pub auto_install: bool,
    pub servers: HashMap<String, Server>,
}
impl Default for Registry {
    fn default() -> Self {
        let mut servers = HashMap::new();
        for (name, languages, command, args, packages) in [
            (
                "typescript",
                vec![
                    "javascript",
                    "javascriptreact",
                    "typescript",
                    "typescriptreact",
                ],
                "typescript-language-server",
                vec!["--stdio"],
                vec!["typescript-language-server@6.0.2", "typescript@7.0.2"],
            ),
            (
                "python",
                vec!["python"],
                "pyright-langserver",
                vec!["--stdio"],
                vec!["pyright@1.1.414"],
            ),
            (
                "html",
                vec!["html"],
                "vscode-html-language-server",
                vec!["--stdio"],
                vec!["vscode-langservers-extracted@4.10.0"],
            ),
            (
                "css",
                vec!["css", "scss", "less"],
                "vscode-css-language-server",
                vec!["--stdio"],
                vec!["vscode-langservers-extracted@4.10.0"],
            ),
            (
                "json",
                vec!["json", "jsonc"],
                "vscode-json-language-server",
                vec!["--stdio"],
                vec!["vscode-langservers-extracted@4.10.0"],
            ),
            ("rust", vec!["rust"], "rust-analyzer", vec![], vec![]),
            ("go", vec!["go"], "gopls", vec![], vec![]),
            (
                "php",
                vec!["php", "blade"],
                "phpactor",
                vec!["language-server"],
                vec![],
            ),
        ] {
            let initialization_options = if name == "php" {
                // Run code actions in-process (item 383). Phpactor's default
                // outsources each codeAction to a spawned process killed after
                // 5s; a timeout yields a silent empty result and a crash a
                // JSON-RPC error, both indistinguishable from "no actions".
                serde_json::json!({
                    "language_server.code_action_outsource": false,
                })
            } else {
                serde_json::json!({})
            };
            servers.insert(
                name.into(),
                Server {
                    languages: languages.into_iter().map(str::to_owned).collect(),
                    command: command.into(),
                    args: args.into_iter().map(str::to_owned).collect(),
                    packages: packages.into_iter().map(str::to_owned).collect(),
                    initialization_options,
                    ..Server::default()
                },
            );
        }
        // Official Laravel language server. Disabled by default: phpactor
        // stays the PHP server until the user switches in
        // language-servers.toml (docs/language-servers.md). Self-contained
        // managed binary, so no php is needed to run the server itself.
        servers.insert(
            "laravel-lsp".into(),
            Server {
                languages: vec!["php".into(), "blade".into()],
                command: "laravel-lsp".into(),
                enabled: false,
                requires_laravel: true,
                initialization_options: serde_json::json!({
                    "phpEnvironment": "auto",
                    "pestGenerateDocBlocks": false,
                }),
                ..Server::default()
            },
        );
        Self {
            enabled: true,
            auto_install: true,
            servers,
        }
    }
}
fn command_exists(command: &str) -> bool {
    let path = std::path::Path::new(command);
    if path.components().count() > 1 || path.is_absolute() {
        return path.is_file();
    }
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .map(str::to_owned)
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&paths).any(|dir| {
        exts.iter()
            .any(|ext| dir.join(format!("{command}{ext}")).is_file())
    })
}
/// Pinned phpactor release. Bump all three together after checking the new
/// asset's digest on the GitHub release page.
pub const PHPACTOR_VERSION: &str = "2026.10.07.0";
const PHPACTOR_URL: &str = "https://github.com/phpactor/phpactor/releases/download/2026.10.07.0/phpactor.phar";
const PHPACTOR_SHA256: &str =
    "b28bf6166e016d539fc2f2aec18ef918b12590e436a6f1ccbdf30b5bbdbdb16b";
fn phpactor_phar() -> Option<PathBuf> {
    Some(
        Directory::data_local_directory()?
            .join("language-servers")
            .join("phpactor")
            .join(format!("phpactor-{PHPACTOR_VERSION}.phar")),
    )
}
/// The stock registry entry (command `phpactor`, no packages). When that
/// command is not on PATH the editor falls back to the managed phar. Any other
/// command, such as a user override, is launched exactly as configured.
fn is_phpactor_default(server: &Server) -> bool {
    server.command == "phpactor"
        && server.packages.is_empty()
        && server.args == ["language-server"]
}
/// Pinned laravel-lsp release (the official Laravel language server). Bump
/// the version and every platform digest together after checking the new
/// assets on the GitHub release page.
pub const LARAVEL_LSP_VERSION: &str = "0.0.32";
const LARAVEL_LSP_X64_LINUX_SHA256: &str =
    "6ff952fa518d8b304fb8a4f8ddd18c3d67358a7fde38358066e5f3e611ea96cf";
const LARAVEL_LSP_ARM64_LINUX_SHA256: &str =
    "fadf14d1e63ba57b72ec43e5ec5b421e284b4595647d1a2b2c2ea3adea942a4c";
const LARAVEL_LSP_X64_DARWIN_SHA256: &str =
    "6894b04d505c9556a59363407bd40da45f60540fb061df8046e52299082962ec";
const LARAVEL_LSP_ARM64_DARWIN_SHA256: &str =
    "61574921e252f477151efc4c6b714bf8175d33e3a0da95ae9bc2a9fd02b69765";
const LARAVEL_LSP_X64_WIN32_SHA256: &str =
    "29880d0d331f82a5af2dfb23d932ee0058bf84c468feb7b70ed0f3888c15c00b";
/// Release asset suffix and digest for this platform, when laravel ships one.
fn laravel_lsp_asset() -> Option<(&'static str, &'static str)> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => Some(("x64-linux", LARAVEL_LSP_X64_LINUX_SHA256)),
        ("aarch64", "linux") => {
            Some(("arm64-linux", LARAVEL_LSP_ARM64_LINUX_SHA256))
        }
        ("x86_64", "macos") => Some(("x64-darwin", LARAVEL_LSP_X64_DARWIN_SHA256)),
        ("aarch64", "macos") => {
            Some(("arm64-darwin", LARAVEL_LSP_ARM64_DARWIN_SHA256))
        }
        ("x86_64", "windows") => {
            Some(("x64-win32.exe", LARAVEL_LSP_X64_WIN32_SHA256))
        }
        _ => None,
    }
}
fn laravel_lsp_binary() -> Option<PathBuf> {
    let extension = if cfg!(windows) { ".exe" } else { "" };
    Some(
        Directory::data_local_directory()?
            .join("language-servers")
            .join("laravel-lsp")
            .join(format!("laravel-lsp-{LARAVEL_LSP_VERSION}{extension}")),
    )
}
/// The stock registry entry (command `laravel-lsp`, no packages). When that
/// command is not on PATH the editor falls back to the managed binary. Any
/// other command, such as a user override, is launched exactly as configured.
fn is_laravel_lsp_default(server: &Server) -> bool {
    server.command == "laravel-lsp"
        && server.packages.is_empty()
        && server.args.is_empty()
}
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn file_sha256(path: &std::path::Path) -> Result<String> {
    Ok(sha256_hex(&std::fs::read(path)?))
}
impl Registry {
    pub fn load() -> Result<Self> {
        let path = Directory::config_directory()
            .context("No config directory")?
            .join("language-servers.toml");
        if !path.exists() {
            let config = Self::default();
            std::fs::write(&path, toml::to_string_pretty(&config)?)?;
            return Ok(config);
        }
        Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
    }
    /// First enabled server claiming the language, alphabetically. Servers
    /// with `requires-laravel` are skipped outside a detected Laravel project
    /// so the next enabled entry (for example phpactor) serves the file.
    pub fn server(
        &self,
        language: &str,
        laravel_project: bool,
    ) -> Option<(String, Server)> {
        if !self.enabled {
            return None;
        }
        let mut names = self.servers.keys().collect::<Vec<_>>();
        names.sort();
        names.into_iter().find_map(|name| {
            let server = &self.servers[name];
            (server.enabled
                && (!server.requires_laravel || laravel_project)
                && server.languages.iter().any(|lang| lang == language))
            .then(|| (name.clone(), server.clone()))
        })
    }
    /// True when launching this server cannot work and would only produce a
    /// spawn error: a directly configured command that is not on PATH, or the
    /// managed phpactor fallback without a usable `php` (or without the phar and
    /// permission to download it). Managed npm servers install on demand and
    /// are never reported missing here.
    pub fn is_missing(&self, server: &Server) -> bool {
        if !server.packages.is_empty() || server.command.is_empty() {
            return false;
        }
        if command_exists(&server.command) {
            return false;
        }
        if is_phpactor_default(server) {
            if !command_exists("php") {
                return true;
            }
            return !self.auto_install
                && !phpactor_phar().is_some_and(|p| p.is_file());
        }
        if is_laravel_lsp_default(server) {
            // The managed binary bundles its own runtime: no php needed.
            return !self.auto_install
                && !laravel_lsp_binary().is_some_and(|p| p.is_file());
        }
        true
    }
    pub fn command(&self, name: &str, server: &Server) -> Result<Command> {
        ensure!(!server.command.is_empty(), "Server command is empty");
        let executable = if !server.packages.is_empty() {
            ensure!(
                !cfg!(windows),
                "Managed npm servers currently support Linux/macOS only; configure a direct node command on Windows"
            );
            ensure!(
                name.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
                "Invalid registry name"
            );
            let root = Directory::data_local_directory()
                .context("No data directory")?
                .join("language-servers")
                .join(name);
            let binary =
                root.join("node_modules")
                    .join(".bin")
                    .join(if cfg!(windows) {
                        format!("{}.cmd", server.command)
                    } else {
                        server.command.clone()
                    });
            ensure!(
                std::path::Path::new(&server.command).components().count() == 1,
                "Invalid managed server command"
            );
            let expected = serde_json::to_vec(&server.packages)?;
            let current = std::fs::read(root.join("editor-packages.json")).ok();
            if !binary.exists() || current.as_ref() != Some(&expected) {
                ensure!(
                    self.auto_install,
                    "Server is not installed and auto-install is disabled"
                );
                self.install(&root, server)?;
            }
            ensure!(
                binary.is_file(),
                "Installed package lacks server executable"
            );
            binary
        } else if !command_exists(&server.command) && is_phpactor_default(server) {
            let phar = self.ensure_phpactor()?;
            let mut command = Command::new("php");
            command.arg(phar).args(&server.args);
            return Ok(command);
        } else if !command_exists(&server.command) && is_laravel_lsp_default(server)
        {
            let binary = self.ensure_laravel_lsp()?;
            let mut command = Command::new(binary);
            command.args(&server.args);
            return Ok(command);
        } else {
            PathBuf::from(&server.command)
        };
        let mut command = Command::new(executable);
        command.args(&server.args);
        Ok(command)
    }
    /// Download the pinned phpactor.phar into the managed directory (once) and
    /// verify its SHA-256 before it is ever executed. A phar already on disk is
    /// re-hashed, so a damaged or swapped file is replaced, never run.
    fn ensure_phpactor(&self) -> Result<PathBuf> {
        let phar = phpactor_phar().context("No data directory")?;
        if phar.is_file()
            && file_sha256(&phar).ok().as_deref() == Some(PHPACTOR_SHA256)
        {
            return Ok(phar);
        }
        ensure!(
            self.auto_install,
            "phpactor is not installed and auto-install is disabled"
        );
        let dir = phar.parent().context("Invalid phpactor directory")?;
        std::fs::create_dir_all(dir)?;
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .user_agent("editor.izdrail.com")
            .build()?;
        let response = client.get(PHPACTOR_URL).send()?;
        ensure!(
            response.status().is_success(),
            "phpactor download failed: HTTP {}",
            response.status()
        );
        let bytes = response.bytes()?;
        ensure!(
            bytes.len() < 64 * 1024 * 1024,
            "phpactor download too large"
        );
        ensure!(
            sha256_hex(&bytes) == PHPACTOR_SHA256,
            "phpactor download failed its SHA-256 check"
        );
        let mut tmp = tempfile::Builder::new()
            .prefix(".phpactor-")
            .tempfile_in(dir)?;
        std::io::Write::write_all(&mut tmp, &bytes)?;
        tmp.persist(&phar)?;
        Ok(phar)
    }
    /// Download the pinned laravel-lsp server binary into the managed
    /// directory (once) and verify its SHA-256 before it is ever executed. A
    /// binary already on disk is re-hashed, so a damaged or swapped file is
    /// replaced, never run. The asset is a self-contained executable; no php
    /// or composer is required to launch the server.
    fn ensure_laravel_lsp(&self) -> Result<PathBuf> {
        let binary = laravel_lsp_binary().context("No data directory")?;
        let (suffix, sha256) = laravel_lsp_asset().context(
            "No managed laravel-lsp binary for this platform; install              laravel-lsp yourself (composer global require laravel/lsp)",
        )?;
        if binary.is_file() && file_sha256(&binary).ok().as_deref() == Some(sha256) {
            return Ok(binary);
        }
        ensure!(
            self.auto_install,
            "laravel-lsp is not installed and auto-install is disabled"
        );
        let dir = binary.parent().context("Invalid laravel-lsp directory")?;
        std::fs::create_dir_all(dir)?;
        let url = format!(
            "https://github.com/laravel/lsp/releases/download/             v{LARAVEL_LSP_VERSION}/server-v{LARAVEL_LSP_VERSION}-{suffix}"
        );
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .user_agent("editor.izdrail.com")
            .build()?;
        let response = client.get(&url).send()?;
        ensure!(
            response.status().is_success(),
            "laravel-lsp download failed: HTTP {}",
            response.status()
        );
        let bytes = response.bytes()?;
        ensure!(
            bytes.len() < 128 * 1024 * 1024,
            "laravel-lsp download too large"
        );
        ensure!(
            sha256_hex(&bytes) == sha256,
            "laravel-lsp download failed its SHA-256 check"
        );
        let mut tmp = tempfile::Builder::new()
            .prefix(".laravel-lsp-")
            .tempfile_in(dir)?;
        std::io::Write::write_all(&mut tmp, &bytes)?;
        tmp.persist(&binary)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                &binary,
                std::fs::Permissions::from_mode(0o755),
            )?;
        }
        Ok(binary)
    }
    fn install(&self, root: &std::path::Path, server: &Server) -> Result<()> {
        let parent = root.parent().context("Invalid managed server directory")?;
        std::fs::create_dir_all(parent)?;
        let lock_path = parent.join(format!(
            ".{}.lock",
            root.file_name()
                .context("Invalid install root")?
                .to_string_lossy()
        ));
        let lock_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .context("Server installation already in progress")?;
        struct Lock(PathBuf, Option<std::fs::File>);
        impl Drop for Lock {
            fn drop(&mut self) {
                drop(self.1.take());
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _lock = Lock(lock_path, Some(lock_file));
        let stage = tempfile::Builder::new()
            .prefix(".server-install-")
            .tempdir_in(parent)?;
        let staged = stage.path().join("new");
        std::fs::create_dir(&staged)?;
        for package in &server.packages {
            let (package_name, version) = package
                .rsplit_once('@')
                .context("Package version must be pinned")?;
            ensure!(
                !package_name.is_empty()
                    && version.split('.').count() >= 3
                    && version.bytes().all(|b| b.is_ascii_digit()
                        || matches!(b, b'.' | b'-' | b'a'..=b'z')),
                "Package version must be exact, not latest/range"
            );
            ensure!(
                !package.starts_with('-')
                    && package.contains('@')
                    && package.len() < 200
                    && package.bytes().all(|b| b.is_ascii_alphanumeric()
                        || matches!(b, b'@' | b'/' | b'-' | b'_' | b'.' | b'+')),
                "Managed packages must be pinned npm name@version entries"
            );
        }
        // Never run dependency lifecycle scripts or install into the user's project.
        let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
        let mut child = Command::new(npm)
            .args(["install", "--prefix"])
            .arg(&staged)
            .args([
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
                "--save-exact",
                "--engine-strict",
            ])
            .args(&server.packages)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("Auto-install needs Node.js and npm on PATH")?;
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(180);
        loop {
            if let Some(status) = child.try_wait()? {
                ensure!(
                    status.success(),
                    "npm language server installation failed (check Node/npm version and connectivity)"
                );
                break;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("Language server install timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        std::fs::write(
            staged.join("editor-packages.json"),
            serde_json::to_vec(&server.packages)?,
        )?;
        let bin = staged.join("node_modules/.bin").join(if cfg!(windows) {
            format!("{}.cmd", server.command)
        } else {
            server.command.clone()
        });
        ensure!(
            bin.is_file(),
            "Package lacks configured language-server binary"
        );
        let backup = stage.path().join("old");
        let existed = root.exists();
        if existed {
            std::fs::rename(root, &backup)?;
        }
        if let Err(error) = std::fs::rename(&staged, root) {
            if existed {
                if let Err(rollback) = std::fs::rename(&backup, root) {
                    let recovery = stage.into_path();
                    anyhow::bail!(
                        "Server install failed: {error}; rollback failed: {rollback}; recovery at {}",
                        recovery.display()
                    );
                }
            }
            return Err(error.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_direct_server_is_detected_and_managed_is_not() {
        let registry = Registry::default();
        let mut server = registry.servers["php"].clone();
        server.command = "definitely-not-a-real-language-server-xyz".into();
        assert!(registry.is_missing(&server));
        server.packages = vec!["x@1.0.0".into()];
        assert!(!registry.is_missing(&server));
        let stock = registry.servers["php"].clone();
        assert!(is_phpactor_default(&stock));
        let mut own = stock.clone();
        own.command = "/opt/my-phpactor".into();
        assert!(!is_phpactor_default(&own));
        assert!(registry.is_missing(&own));
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let mut sh = registry.servers["php"].clone();
        sh.command = "sh".into();
        if cfg!(unix) {
            assert!(!registry.is_missing(&sh));
        }
    }

    #[test]
    fn auto_install_can_be_disabled_and_custom_commands_need_no_installer() {
        let registry = Registry {
            auto_install: false,
            ..Registry::default()
        };
        let command = Server {
            command: "python3".into(),
            args: vec!["--version".into()],
            ..Server::default()
        };
        assert_eq!(
            registry.command("custom", &command).unwrap().get_program(),
            "python3"
        );
        assert!(
            registry
                .command("../escape", &registry.servers["python"])
                .is_err()
        );
    }
    #[test]
    #[ignore = "explicit pinned npm install smoke in temporary directory"]
    fn actual_npm_auto_install() {
        let registry = Registry::default();
        let server = &registry.servers["python"];
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("pyright");
        registry.install(&destination, server).unwrap();
        assert!(
            destination
                .join("node_modules/.bin/pyright-langserver")
                .is_file()
        );
    }
    #[test]
    fn laravel_lsp_entry_is_seeded_disabled_and_gated() {
        let registry = Registry::default();
        let server = &registry.servers["laravel-lsp"];
        assert!(!server.enabled);
        assert!(server.requires_laravel);
        assert_eq!(server.languages, ["php", "blade"]);
        assert_eq!(server.command, "laravel-lsp");
        assert_eq!(server.initialization_options["phpEnvironment"], "auto");
        assert_eq!(
            server.initialization_options["pestGenerateDocBlocks"],
            false
        );
        assert!(is_laravel_lsp_default(server));
        // phpactor stays the PHP server, in and out of Laravel projects.
        assert_eq!(registry.server("php", true).unwrap().0, "php");
        assert_eq!(registry.server("blade", false).unwrap().0, "php");
        let mut own = server.clone();
        own.command = "/opt/laravel-lsp".into();
        assert!(!is_laravel_lsp_default(&own));
        assert!(registry.is_missing(&own));
    }
    #[test]
    fn enabled_laravel_lsp_wins_in_laravel_and_falls_back_elsewhere() {
        let mut registry = Registry::default();
        registry.servers.get_mut("laravel-lsp").unwrap().enabled = true;
        assert_eq!(registry.server("php", true).unwrap().0, "laravel-lsp");
        assert_eq!(registry.server("blade", true).unwrap().0, "laravel-lsp");
        // Outside a Laravel project the gated entry falls through to phpactor.
        assert_eq!(registry.server("php", false).unwrap().0, "php");
        registry.servers.get_mut("php").unwrap().enabled = false;
        assert!(registry.server("php", false).is_none());
        assert_eq!(registry.server("php", true).unwrap().0, "laravel-lsp");
        // The managed default bundles its runtime: installable without php.
        let stock = registry.servers["laravel-lsp"].clone();
        assert!(!registry.is_missing(&stock));
    }
}
