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
            servers.insert(
                name.into(),
                Server {
                    languages: languages.into_iter().map(str::to_owned).collect(),
                    command: command.into(),
                    args: args.into_iter().map(str::to_owned).collect(),
                    packages: packages.into_iter().map(str::to_owned).collect(),
                    ..Server::default()
                },
            );
        }
        Self {
            enabled: true,
            auto_install: true,
            servers,
        }
    }
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
    pub fn server(&self, language: &str) -> Option<(String, Server)> {
        if !self.enabled {
            return None;
        }
        let mut names = self.servers.keys().collect::<Vec<_>>();
        names.sort();
        names.into_iter().find_map(|name| {
            let server = &self.servers[name];
            (server.enabled && server.languages.iter().any(|lang| lang == language))
                .then(|| (name.clone(), server.clone()))
        })
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
        } else {
            PathBuf::from(&server.command)
        };
        let mut command = Command::new(executable);
        command.args(&server.args);
        Ok(command)
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
}
