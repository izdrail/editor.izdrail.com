//! Bounded data-only package installation. No executable plugin path is used.
use anyhow::{Context, Result, bail, ensure};
use lapce_core::directory::Directory;
use lapce_rpc::plugin::{VoltInfo, VoltMetadata};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path},
    time::Duration,
};
const MAX_DOWNLOAD: u64 = 32 * 1024 * 1024;
const MAX_EXTRACTED: u64 = 128 * 1024 * 1024;
const MAX_FILES: usize = 20000;
const STORAGE_HOST: &str =
    "f44c0ff23efc529a84c18aa5af37dd81.r2.cloudflarestorage.com";

fn identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "Invalid package identifier"
    );
    Ok(())
}
fn relative(path: &Path) -> Result<()> {
    ensure!(!path.as_os_str().is_empty(), "Empty resource path");
    ensure!(
        path.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir)),
        "Unsafe resource path"
    );
    ensure!(
        !path.to_string_lossy().contains('\\')
            && !path.to_string_lossy().contains(':'),
        "Non-portable resource path"
    );
    Ok(())
}
fn resource(root: &Path, name: &str) -> Result<std::path::PathBuf> {
    let path = Path::new(name);
    relative(path)?;
    let resolved = root
        .join(path)
        .canonicalize()
        .context("Missing package resource")?;
    ensure!(
        resolved.starts_with(root.canonicalize()?) && resolved.is_file(),
        "Resource outside package"
    );
    Ok(resolved)
}
fn read_bounded(mut reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.by_ref().take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "Package exceeds size limit");
    Ok(bytes)
}
fn validate_url(url: &url::Url) -> Result<()> {
    ensure!(
        url.scheme() == "https"
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none(),
        "Unsafe download URL"
    );
    ensure!(
        matches!(url.host_str(), Some("plugins.lapce.dev"))
            || url.host_str() == Some(STORAGE_HOST),
        "Untrusted download host"
    );
    Ok(())
}
fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        // Signed archive URLs are returned as text. Never follow arbitrary redirects.
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}
fn fetch(
    client: &reqwest::blocking::Client,
    url: &url::Url,
    limit: u64,
) -> Result<Vec<u8>> {
    validate_url(url)?;
    let response = client.get(url.clone()).send()?.error_for_status()?;
    ensure!(
        !response.status().is_redirection(),
        "Download redirects are not allowed"
    );
    read_bounded(response, limit)
}

pub fn download_volt(volt: &VoltInfo) -> Result<VoltMetadata> {
    ensure!(!volt.wasm, "Only color and icon themes can be installed");
    identifier(&volt.author)?;
    identifier(&volt.name)?;
    ensure!(
        !volt.version.is_empty()
            && volt.version.len() < 128
            && volt.version.bytes().all(|b| b.is_ascii_alphanumeric()
                || matches!(b, b'.' | b'-' | b'_' | b'+')),
        "Invalid package version"
    );
    let client = client()?;
    let endpoint = url::Url::parse(&format!(
        "https://plugins.lapce.dev/api/v1/plugins/{}/{}/{}/download",
        volt.author, volt.name, volt.version
    ))?;
    let address = fetch(&client, &endpoint, 8192)?;
    let url = url::Url::parse(std::str::from_utf8(&address)?.trim())?;
    let archive = fetch(&client, &url, MAX_DOWNLOAD)?;
    let root = Directory::plugins_directory().context("No plugin directory")?;
    install_archive(volt, &archive, &root)
}

fn unpack(reader: impl Read, stage: &Path) -> Result<()> {
    let mut archive =
        tar::Archive::new(reader.take(MAX_EXTRACTED + 16 * 1024 * 1024));
    let mut size = 0u64;
    for (count, entry) in archive.entries()?.enumerate() {
        ensure!(count < MAX_FILES, "Too many package files");
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        relative(&path)?;
        let kind = entry.header().entry_type();
        ensure!(
            kind.is_file() || kind.is_dir(),
            "Links and special archive entries are forbidden"
        );
        if kind.is_dir() {
            fs::create_dir_all(stage.join(&path))?;
            continue;
        }
        size = size
            .checked_add(entry.size())
            .context("Invalid archive size")?;
        ensure!(
            size <= MAX_EXTRACTED,
            "Extracted package exceeds size limit"
        );
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        ensure!(
            matches!(
                ext.as_str(),
                "toml"
                    | "svg"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "webp"
                    | "gif"
                    | "ico"
                    | "md"
                    | "txt"
            ) || matches!(
                path.file_name().and_then(|s| s.to_str()),
                Some("LICENSE" | "LICENCE")
            ),
            "Non-data package file: {}",
            path.display()
        );
        let destination = stage.join(&path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        std::io::copy(&mut entry, &mut output)?;
        output.flush()?;
    }
    Ok(())
}

fn validate(stage: &Path, volt: &VoltInfo) -> Result<VoltMetadata> {
    let raw = fs::read_to_string(stage.join("volt.toml"))?;
    let manifest: toml::Value = toml::from_str(&raw)?;
    // Inspect raw TOML, before load_volt can erase a missing WASM path.
    ensure!(
        manifest.get("wasm").is_none(),
        "Executable WASM declarations are forbidden"
    );
    let meta: VoltMetadata = toml::from_str(&raw)?;
    ensure!(
        meta.name == volt.name
            && meta.author == volt.author
            && meta.version == volt.version,
        "Package identity does not match selected item"
    );
    let mut themes = 0;
    for (files, section) in [
        (&meta.color_themes, "color-theme"),
        (&meta.icon_themes, "icon-theme"),
    ] {
        for name in files.iter().flatten() {
            let path = resource(stage, name)?;
            let theme: toml::Value = toml::from_str(&fs::read_to_string(&path)?)?;
            let table = theme
                .get(section)
                .and_then(|v| v.as_table())
                .context("Invalid theme table")?;
            ensure!(
                table
                    .get("name")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.trim().is_empty()),
                "Theme has no name"
            );
            if section == "icon-theme" {
                for key in ["ui", "foldername", "filename", "extension"] {
                    if let Some(refs) = table.get(key).and_then(|v| v.as_table()) {
                        for value in refs.values() {
                            let name =
                                value.as_str().context("Invalid icon resource")?;
                            // Theme references are relative to the theme file's directory.
                            let target = resource(path.parent().unwrap(), name)?;
                            ensure!(
                                target.starts_with(stage.canonicalize()?),
                                "Icon outside package"
                            );
                        }
                    }
                }
            }
            themes += 1;
        }
    }
    ensure!(themes > 0, "Package contains no usable color/icon theme");
    if let Some(icon) = &meta.icon {
        resource(stage, icon)?;
    }
    Ok(meta)
}

fn install_archive(
    volt: &VoltInfo,
    bytes: &[u8],
    root: &Path,
) -> Result<VoltMetadata> {
    ensure!(!volt.wasm, "Executable plugins cannot be installed");
    identifier(&volt.author)?;
    identifier(&volt.name)?;
    fs::create_dir_all(root)?;
    let lock_path = root.join(format!(".{}.install-lock", volt.id()));
    let lock_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .context("Another installation is in progress; retry after it finishes")?;
    struct Lock(std::path::PathBuf, Option<fs::File>);
    impl Drop for Lock {
        fn drop(&mut self) {
            drop(self.1.take());
            let _ = fs::remove_file(&self.0);
        }
    }
    let _lock = Lock(lock_path, Some(lock_file));
    let staging = tempfile::Builder::new()
        .prefix(".theme-install-")
        .tempdir_in(root)?;
    let stage = staging.path().join("new");
    fs::create_dir(&stage)?;
    if bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        unpack(zstd::Decoder::new(bytes)?, &stage)?;
    } else if bytes.starts_with(&[0x1f, 0x8b]) {
        unpack(flate2::read::GzDecoder::new(bytes), &stage)?;
    } else {
        bail!("Unsupported theme archive");
    }
    validate(&stage, volt)?;
    let target = root.join(volt.id().to_string());
    let backup = staging.path().join("old");
    let exists = target.try_exists()?;
    if exists {
        ensure!(
            fs::symlink_metadata(&target)?.is_dir(),
            "Invalid installed package directory"
        );
        fs::rename(&target, &backup)?;
    }
    if let Err(error) = fs::rename(&stage, &target) {
        if exists {
            if let Err(rollback) = fs::rename(&backup, &target) {
                // Keep recovery bytes even if the second rename unexpectedly fails.
                let recovery = staging.into_path();
                bail!(
                    "Install failed: {error}; rollback failed: {rollback}; previous theme kept at {}",
                    recovery.display()
                );
            }
        }
        return Err(error.into());
    }
    match super::load_volt(&target) {
        Ok(meta) => Ok(meta),
        Err(error) => {
            let rejected = staging.path().join("rejected");
            let rollback = fs::rename(&target, &rejected).and_then(|_| {
                if exists {
                    fs::rename(&backup, &target)
                } else {
                    Ok(())
                }
            });
            if let Err(rollback) = rollback {
                let recovery = staging.into_path();
                bail!(
                    "Post-install load failed: {error}; rollback failed: {rollback}; recovery at {}",
                    recovery.display()
                );
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn info() -> VoltInfo {
        VoltInfo {
            name: "theme".into(),
            author: "author".into(),
            version: "1.0".into(),
            display_name: "Theme".into(),
            description: "Test".into(),
            repository: None,
            wasm: false,
            updated_at_ts: 0,
        }
    }
    fn manifest(extra: &str) -> String {
        format!(
            "name = 'theme'\nauthor = 'author'\nversion = '1.0'\ndisplay-name = 'Theme'\ndescription = 'Test'\ncolor-themes = ['theme.toml']\n{extra}\n"
        )
    }
    fn archive(files: &[(&str, &str)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (path, data) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, path, data.as_bytes())
                .unwrap();
        }
        let tar = builder.into_inner().unwrap();
        zstd::encode_all(tar.as_slice(), 1).unwrap()
    }
    const THEME: &str = "[color-theme]\nname = 'Test Theme'\n[color-theme.base]\nblack = '#000000'\n";
    #[test]
    fn installed_theme_resolves_through_existing_loader_and_updates() {
        let root = tempfile::tempdir().unwrap();
        let bytes = archive(&[("volt.toml", &manifest("")), ("theme.toml", THEME)]);
        let meta = install_archive(&info(), &bytes, root.path()).unwrap();
        assert!(meta.wasm.is_none());
        let path = &meta.color_themes.as_ref().unwrap()[0];
        assert!(Path::new(path).is_file());
        let theme: toml::Value =
            toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(theme["color-theme"]["name"].as_str(), Some("Test Theme"));
        install_archive(&info(), &bytes, root.path()).unwrap();
        assert!(super::super::load_volt(meta.dir.as_ref().unwrap()).is_ok());
    }
    #[test]
    fn wasm_flag_and_missing_wasm_declaration_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let mut executable = info();
        executable.wasm = true;
        assert!(install_archive(&executable, &[], root.path()).is_err());
        let bytes = archive(&[
            ("volt.toml", &manifest("wasm = 'missing.wasm'")),
            ("theme.toml", THEME),
        ]);
        assert!(install_archive(&info(), &bytes, root.path()).is_err());
        assert!(!root.path().join("author.theme").exists());
        let local = root.path().join("local");
        fs::create_dir(&local).unwrap();
        fs::write(local.join("volt.toml"), manifest("wasm = 'missing.wasm'"))
            .unwrap();
        assert!(super::super::load_volt(&local).is_err());
    }
    #[test]
    fn failed_update_keeps_old_package_and_releases_lock() {
        let root = tempfile::tempdir().unwrap();
        let good = archive(&[("volt.toml", &manifest("")), ("theme.toml", THEME)]);
        install_archive(&info(), &good, root.path()).unwrap();
        let bad =
            archive(&[("volt.toml", &manifest("")), ("theme.toml", "broken TOML!")]);
        assert!(install_archive(&info(), &bad, root.path()).is_err());
        assert_eq!(
            fs::read_to_string(root.path().join("author.theme/theme.toml")).unwrap(),
            THEME
        );
        assert!(install_archive(&info(), &good, root.path()).is_ok());
    }
    #[test]
    fn identity_resources_and_executable_files_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        for files in [
            vec![
                (
                    "volt.toml",
                    manifest("").replace("name = 'theme'", "name = 'spoof'"),
                ),
                ("theme.toml", THEME.into()),
            ],
            vec![
                (
                    "volt.toml",
                    manifest("").replace("theme.toml", "../escape.toml"),
                ),
                ("theme.toml", THEME.into()),
            ],
            vec![
                ("volt.toml", manifest("")),
                ("theme.toml", THEME.into()),
                ("run.wasm", "code".into()),
            ],
        ] {
            let refs = files
                .iter()
                .map(|(p, s)| (*p, s.as_str()))
                .collect::<Vec<_>>();
            assert!(install_archive(&info(), &archive(&refs), root.path()).is_err());
        }
    }
    #[test]
    fn symlinks_hardlinks_and_special_entries_are_rejected() {
        let stage = tempfile::tempdir().unwrap();
        for kind in [
            tar::EntryType::Symlink,
            tar::EntryType::Link,
            tar::EntryType::Fifo,
        ] {
            let mut builder = tar::Builder::new(Vec::new());
            let mut header = tar::Header::new_gnu();
            header.set_size(0);
            header.set_mode(0o644);
            header.set_entry_type(kind);
            header.set_link_name("outside").unwrap();
            header.set_cksum();
            builder
                .append_data(&mut header, "link", std::io::empty())
                .unwrap();
            assert!(
                unpack(builder.into_inner().unwrap().as_slice(), stage.path())
                    .is_err()
            );
        }
    }
    #[test]
    fn paths_hosts_and_read_limits_are_enforced() {
        for path in ["../file", "/absolute", "C:\\file", "folder/../../file"] {
            assert!(relative(Path::new(path)).is_err());
        }
        for address in [
            "http://plugins.lapce.dev/x",
            "https://127.0.0.1/x",
            "https://plugins.lapce.dev.evil/x",
            "https://user@plugins.lapce.dev/x",
            "https://plugins.lapce.dev:8443/x",
        ] {
            assert!(validate_url(&url::Url::parse(address).unwrap()).is_err());
        }
        assert!(
            validate_url(&url::Url::parse("https://plugins.lapce.dev/x").unwrap())
                .is_ok()
        );
        assert!(read_bounded(&b"12345"[..], 4).is_err());
        assert_eq!(read_bounded(&b"1234"[..], 4).unwrap(), b"1234");
    }
    #[test]
    fn no_theme_or_missing_resource_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        assert!(
            install_archive(
                &info(),
                &archive(&[("volt.toml", &manifest(""))]),
                root.path()
            )
            .is_err()
        );
        let empty = manifest("").replace("color-themes = ['theme.toml']", "");
        assert!(
            install_archive(
                &info(),
                &archive(&[("volt.toml", &empty)]),
                root.path()
            )
            .is_err()
        );
    }
    #[test]
    fn truncated_archive_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let good = archive(&[("volt.toml", &manifest("")), ("theme.toml", THEME)]);
        assert!(
            install_archive(&info(), &good[..good.len() / 2], root.path()).is_err()
        );
    }
}

#[cfg(test)]
mod registry_smoke {
    use super::*;
    #[test]
    #[ignore = "explicit read-only registry download smoke test"]
    fn actual_registry_theme_archive() {
        let path = std::env::var("LAPCE_THEME_TEST_ARCHIVE").unwrap();
        let raw = fs::read(path).unwrap();
        let info: VoltInfo =
            serde_json::from_str(&std::env::var("LAPCE_THEME_TEST_INFO").unwrap())
                .unwrap();
        let root = tempfile::tempdir().unwrap();
        let meta = install_archive(&info, &raw, root.path()).unwrap();
        assert!(meta.wasm.is_none());
        if let Ok(destination) = std::env::var("LAPCE_THEME_TEST_DESTINATION") {
            install_archive(&info, &raw, Path::new(&destination)).unwrap();
        }
        assert!(
            meta.color_themes.as_ref().is_some_and(|v| !v.is_empty())
                || meta.icon_themes.as_ref().is_some_and(|v| !v.is_empty())
        );
    }
}

#[cfg(test)]
mod archive_limits {
    use super::*;
    #[test]
    fn oversized_header_and_duplicate_file_are_rejected() {
        let stage = tempfile::tempdir().unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_path("huge.txt").unwrap();
        header.set_size(MAX_EXTRACTED + 1);
        header.set_mode(0o644);
        header.set_cksum();
        let mut bytes = header.as_bytes().to_vec();
        bytes.extend([0u8; 1024]);
        assert!(unpack(bytes.as_slice(), stage.path()).is_err());
        let mut builder = tar::Builder::new(Vec::new());
        for _ in 0..2 {
            let mut header = tar::Header::new_gnu();
            header.set_size(1);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "duplicate.txt", &b"x"[..])
                .unwrap();
        }
        assert!(
            unpack(builder.into_inner().unwrap().as_slice(), stage.path()).is_err()
        );
    }
}
