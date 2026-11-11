//! Static project evidence shared by syntax and completion. Never executes PHP.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectProfile {
    pub root: Option<PathBuf>,
    pub laravel: bool,
    pub symfony_app: bool,
    pub wordpress: bool,
    pub wordpress_version: Option<String>,
    pub packages: BTreeMap<String, String>,
}

// Reject special files before opening them, then recheck the opened handle.
// Reads remain bounded even if a regular file grows after metadata is checked.
fn read_regular(path: &Path, limit: u64) -> Option<Vec<u8>> {
    use std::io::Read;
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn json(path: &Path) -> Option<serde_json::Value> {
    let bytes = read_regular(path, 1 << 20)?;
    if bytes.len() > 1 << 20 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

pub fn detect(file: &Path) -> ProjectProfile {
    let mut profile = ProjectProfile::default();
    let Some(dir) = file.parent() else {
        return profile;
    };
    // Nearest Composer root is a boundary, even if its JSON is invalid.
    // No TTL cache: marker removal and lockfile edits apply on the next read.
    for root in dir.ancestors().take(24) {
        let composer = root.join("composer.json");
        let header = |file: &Path, marker: &str| -> bool {
            let Some(mut bytes) = read_regular(file, 8192) else {
                return false;
            };
            bytes.truncate(8192);
            std::str::from_utf8(&bytes).is_ok_and(|s| s.contains(marker))
        };
        let plugin_file = |path: &Path| {
            path.extension().is_some_and(|e| e == "php")
                && std::fs::metadata(path)
                    .is_ok_and(|m| m.is_file() && m.len() <= 1 << 20)
                && header(path, "Plugin Name:")
        };
        // The opened file is direct evidence, not an arbitrary neighbor. Keep
        // the neighbor scan bounded without losing it in a large directory.
        let plugin = (file.parent() == Some(root) && plugin_file(file))
            || std::fs::read_dir(root).ok().is_some_and(|files| {
                files.take(128).filter_map(Result::ok).any(|f| {
                    f.path() != file && plugin_file(&f.path())
                })
            });
        let wordpress = plugin
            || header(&root.join("style.css"), "Theme Name:")
            || root.join("wp-config.php").is_file()
            || root.join("wp-content").is_dir()
            || (root.join("wp-includes/version.php").is_file()
                && root.join("wp-admin").is_dir());
        if composer.is_file()
            || root.join("artisan").is_file()
            || root.join("symfony.lock").is_file()
            || wordpress
        {
            profile.root = Some(root.to_owned());
            profile.wordpress = wordpress;
            let version_file = root.join("wp-includes/version.php");
            if let Some(version) = read_regular(&version_file, 8192)
                .filter(|bytes| bytes.len() <= 8192)
                .and_then(|bytes| String::from_utf8(bytes).ok())
            {
                let re = regex::Regex::new(r#"\$wp_version\s*=\s*['"]([^'"]+)['"]"#)
                    .unwrap();
                profile.wordpress_version =
                    re.captures(&version).map(|c| c[1].to_owned());
            }
            if let Some(value) = json(&composer) {
                for section in ["require", "require-dev"] {
                    if let Some(packages) = value[section].as_object() {
                        for (name, version) in packages {
                            if let Some(version) = version.as_str() {
                                profile
                                    .packages
                                    .insert(name.clone(), version.to_owned());
                            }
                        }
                    }
                }
            }
            if let Some(lock) = json(&root.join("composer.lock")) {
                for section in ["packages", "packages-dev"] {
                    if let Some(packages) = lock[section].as_array() {
                        for package in packages {
                            if let (Some(name), Some(version)) = (
                                package["name"].as_str(),
                                package["version"].as_str(),
                            ) {
                                profile
                                    .packages
                                    .insert(name.to_owned(), version.to_owned());
                            }
                        }
                    }
                }
            }
            profile.laravel = root.join("artisan").is_file()
                || profile.packages.contains_key("laravel/framework");
            profile.symfony_app = root.join("symfony.lock").is_file()
                || profile.packages.contains_key("symfony/framework-bundle");
            profile.wordpress |= profile.packages.keys().any(|p| {
                p == "johnpbloch/wordpress"
                    || p == "roots/wordpress"
                    || p == "wordpress/core"
            });
            return profile;
        }
    }
    profile
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearest_root_versions_combinations_and_removal() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("child/src")).unwrap();
        std::fs::write(root.join("composer.json"), r#"{"require":{"laravel/framework":"^12", "symfony/framework-bundle":"^7"}}"#).unwrap();
        std::fs::create_dir(root.join("wp-content")).unwrap();
        let file = root.join("src/x.php");
        let p = detect(&file);
        assert!(p.laravel && p.symfony_app && p.wordpress);
        std::fs::write(root.join("composer.lock"), r#"{"packages":[{"name":"symfony/framework-bundle","version":"v7.3.1"}]}"#).unwrap();
        assert_eq!(detect(&file).packages["symfony/framework-bundle"], "v7.3.1");
        std::fs::write(root.join("child/composer.json"), "invalid").unwrap();
        let child = detect(&root.join("child/src/x.php"));
        assert!(!child.laravel && !child.wordpress);
        assert_eq!(child.root.as_deref(), Some(root.join("child").as_path()));
        std::fs::remove_file(root.join("composer.json")).unwrap();
        std::fs::remove_dir(root.join("wp-content")).unwrap();
        assert!(detect(&file).root.is_none());
    }
    #[test]
    fn component_is_not_application() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("composer.json"),
            r#"{"require":{"symfony/console":"^7"}}"#,
        )
        .unwrap();
        let p = detect(&tmp.path().join("x.php"));
        assert!(!p.symfony_app);
        assert!(p.packages.contains_key("symfony/console"));
    }
}

#[cfg(test)]
mod wordpress_tests {
    use super::*;
    #[test]
    fn standalone_plugins_themes_and_version_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("plugin.php"), "<?php /* Plugin Name: Demo */")
            .unwrap();
        assert!(detect(&root.join("plugin.php")).wordpress);
        std::fs::remove_file(root.join("plugin.php")).unwrap();
        std::fs::write(root.join("style.css"), "/* Theme Name: Demo */").unwrap();
        assert!(detect(&root.join("functions.php")).wordpress);
        std::fs::create_dir_all(root.join("wp-includes")).unwrap();
        std::fs::write(
            root.join("wp-includes/version.php"),
            "<?php $wp_version = '6.8.3';",
        )
        .unwrap();
        let p = detect(&root.join("functions.php"));
        assert_eq!(p.wordpress_version.as_deref(), Some("6.8.3"));
        assert!(
            crate::web_metadata::php_symbols(&p).any(|s| s.name == "add_action")
        );
        std::fs::write(
            root.join("wp-includes/version.php"),
            "<?php $wp_version = '5.0';",
        )
        .unwrap();
        assert_eq!(
            crate::web_metadata::php_symbols(&detect(&root.join("functions.php")))
                .filter(|s| s.framework != "php")
                .count(),
            0
        );
    }
}

#[cfg(all(test, unix))]
mod special_file_tests {
    use super::*;
    #[test]
    fn project_detection_ignores_fifo_headers_and_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        for name in [
            "plugin.php",
            "style.css",
            "composer.json",
            "composer.lock",
            "wp-includes/version.php",
        ] {
            let path = tmp.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            assert!(std::process::Command::new("mkfifo")
                .arg(&path)
                .status()
                .unwrap()
                .success());
        }
        std::fs::write(tmp.path().join("artisan"), "").unwrap();
        // No writer exists: opening any of these FIFOs for reading would hang.
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            sender.send(detect(&tmp.path().join("index.php"))).unwrap();
        });
        let profile = receiver
            .recv_timeout(std::time::Duration::from_secs(3))
            .expect("project detection blocked on a named pipe");
        assert!(!profile.wordpress);
        assert!(profile.packages.is_empty());
        assert!(profile.wordpress_version.is_none());
    }
}

#[cfg(test)]
mod bounded_read_tests {
    use super::*;
    #[test]
    fn regular_reads_keep_prefixes_but_reject_oversized_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("artisan"), "").unwrap();
        let mut composer = br#"{"require":{"laravel/framework":"^12"}}"#.to_vec();
        composer.resize(1 << 20, b' ');
        std::fs::write(root.join("composer.json"), &composer).unwrap();
        assert!(json(&root.join("composer.json")).is_some());
        composer.push(b' ');
        std::fs::write(root.join("composer.json"), &composer).unwrap();
        assert!(json(&root.join("composer.json")).is_none());
        let mut header = b"/* Theme Name: Demo */".to_vec();
        header.resize(16384, b' ');
        std::fs::write(root.join("style.css"), header).unwrap();
        assert!(detect(&root.join("index.php")).wordpress);
        std::fs::create_dir(root.join("wp-includes")).unwrap();
        let mut version = b"<?php $wp_version = '6.8.3';".to_vec();
        version.resize(8192, b' ');
        std::fs::write(root.join("wp-includes/version.php"), &version).unwrap();
        assert_eq!(
            detect(&root.join("index.php")).wordpress_version.as_deref(),
            Some("6.8.3")
        );
        version.push(b' ');
        std::fs::write(root.join("wp-includes/version.php"), version).unwrap();
        assert!(detect(&root.join("index.php")).wordpress_version.is_none());
        assert!(read_regular(root, 8192).is_none());
    }
    #[cfg(unix)]
    #[test]
    fn regular_symlinks_still_supply_project_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path();
        std::fs::write(
            path.join("manifest"),
            r#"{"require":{"symfony/framework-bundle":"7.3.0"}}"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(
            path.join("manifest"),
            path.join("composer.json"),
        )
        .unwrap();
        assert!(detect(&path.join("index.php")).symfony_app);
    }
}

#[cfg(test)]
mod opened_plugin_tests {
    use super::*;
    #[test]
    fn opened_plugin_outside_neighbor_scan_is_still_recognized() {
        let tmp = tempfile::tempdir().unwrap();
        for index in 0..256 {
            std::fs::write(tmp.path().join(format!("entry{index}.php")), "<?php").unwrap();
        }
        // Select an actual entry beyond the bounded neighbor scan, independent
        // of the filesystem's directory ordering.
        let plugin = std::fs::read_dir(tmp.path())
            .unwrap()
            .nth(200)
            .unwrap()
            .unwrap()
            .path();
        std::fs::write(&plugin, "<?php /* Plugin Name: Opened Plugin */").unwrap();
        let profile = detect(&plugin);
        assert!(profile.wordpress);
        assert_eq!(profile.root.as_deref(), Some(tmp.path()));
        assert!(crate::web_metadata::php_symbols(&profile)
            .any(|s| s.name == "add_action"));
    }
}
