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

fn json(path: &Path) -> Option<serde_json::Value> {
    // Bound before reading, not after allocating an arbitrarily large file.
    if std::fs::metadata(path).ok()?.len() > 1 << 20 {
        return None;
    }
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
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
            use std::io::Read;
            let Ok(file) = std::fs::File::open(file) else {
                return false;
            };
            let mut bytes = Vec::new();
            if file.take(8192).read_to_end(&mut bytes).is_err() {
                return false;
            }
            std::str::from_utf8(&bytes).is_ok_and(|s| s.contains(marker))
        };
        let plugin = std::fs::read_dir(root).ok().is_some_and(|files| {
            files.take(128).filter_map(Result::ok).any(|f| {
                let path = f.path();
                path.extension().is_some_and(|e| e == "php")
                    && std::fs::metadata(&path).is_ok_and(|m| m.len() <= 1 << 20)
                    && header(&path, "Plugin Name:")
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
            if let Some(version) = std::fs::metadata(&version_file)
                .ok()
                .filter(|m| m.len() <= 8192)
                .and_then(|_| std::fs::read_to_string(version_file).ok())
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
                .count(),
            0
        );
    }
}
