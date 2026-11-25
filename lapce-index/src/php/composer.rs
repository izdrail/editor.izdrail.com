//! Declarative Composer metadata only. Never loads PHP/autoload code.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

#[derive(Default, Debug, Clone)]
struct Trie {
    dirs: Vec<PathBuf>,
    children: BTreeMap<String, Trie>,
}
impl Trie {
    fn insert(&mut self, prefix: &str, dirs: Vec<PathBuf>) {
        let mut at = self;
        for part in prefix
            .trim_matches('\\')
            .split('\\')
            .filter(|s| !s.is_empty())
        {
            at = at.children.entry(part.to_owned()).or_default();
        }
        at.dirs.extend(dirs);
    }
    fn candidates(&self, name: &str) -> Vec<PathBuf> {
        let parts = name
            .trim_start_matches('\\')
            .split('\\')
            .collect::<Vec<_>>();
        let mut levels = vec![(self, 0)];
        let mut at = self;
        for (i, part) in parts.iter().enumerate().take(parts.len().saturating_sub(1))
        {
            let Some(child) = at.children.get(*part) else {
                break;
            };
            at = child;
            levels.push((at, i + 1));
        }
        let mut result = Vec::new();
        for (node, n) in levels.into_iter().rev() {
            for dir in &node.dirs {
                let mut p = dir.clone();
                for part in &parts[n..] {
                    p.push(part);
                }
                p.set_extension("php");
                result.push(p);
            }
        }
        result
    }
}
#[derive(Default, Debug, Clone)]
pub struct Composer {
    trie: Trie,
    pub roots: Vec<PathBuf>,
    pub classmap_roots: Vec<PathBuf>,
    pub workspace: PathBuf,
    pub vendor: PathBuf,
    pub files: Vec<PathBuf>,
    pub manifests: Vec<PathBuf>,
    pub excludes: Vec<(PathBuf, String)>,
    pub packages: Vec<String>,
    pub fingerprint: String,
    pub warnings: Vec<String>,
    compiled_excludes: Vec<(PathBuf, globset::GlobMatcher)>,
}
fn strings(v: &Value) -> Vec<&str> {
    match v {
        Value::String(s) => vec![s],
        Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    }
}
fn bounded_json(path: &Path) -> io::Result<(Value, Vec<u8>)> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(io::Error::other("Composer metadata exceeds 8 MiB"));
    }
    let v = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    Ok((v, bytes))
}
impl Composer {
    pub fn resolve(&self, name: &str) -> Vec<PathBuf> {
        if name
            .split('\\')
            .any(|s| s == ".." || s == "." || s.contains('/'))
        {
            return vec![];
        }
        self.trie.candidates(name)
    }
    fn safe(&mut self, base: &Path, value: &str) -> Option<PathBuf> {
        if Path::new(value).is_absolute() {
            self.warnings
                .push(format!("rejected absolute Composer path {value}"));
            return None;
        }
        let path = base.join(value);
        let mut ancestor = path.as_path();
        let mut tail = Vec::new();
        while !ancestor.exists() {
            let Some(name) = ancestor.file_name() else {
                return None;
            };
            tail.push(name.to_os_string());
            ancestor = ancestor.parent()?;
        }
        let mut canonical = ancestor.canonicalize().ok()?;
        for part in tail.into_iter().rev() {
            canonical.push(part);
        }
        // Resolve dot components even when a declared directory is not present.
        let mut normalized = PathBuf::new();
        for c in canonical.components() {
            match c {
                std::path::Component::ParentDir => {
                    normalized.pop();
                }
                std::path::Component::CurDir => {}
                _ => normalized.push(c),
            }
        }
        if !normalized.starts_with(&self.workspace) {
            self.warnings.push(format!(
                "rejected outside-workspace Composer path {}",
                path.display()
            ));
            None
        } else {
            Some(normalized)
        }
    }
    fn autoload(&mut self, base: &Path, v: &Value, dev: bool) {
        for key in if dev {
            vec!["autoload", "autoload-dev"]
        } else {
            vec!["autoload"]
        } {
            let Some(autoload) = v.get(key) else { continue };
            if let Some(psr) = autoload.get("psr-4").and_then(Value::as_object) {
                for (prefix, dirs) in psr {
                    // Composer requires a terminal separator for non-empty prefixes.
                    if !prefix.is_empty() && !prefix.ends_with('\\') {
                        self.warnings.push(format!("invalid PSR-4 prefix {prefix}"));
                        continue;
                    }
                    let dirs = strings(dirs)
                        .into_iter()
                        .filter_map(|s| self.safe(base, s))
                        .collect::<Vec<_>>();
                    self.roots.extend(dirs.clone());
                    self.trie.insert(prefix, dirs);
                }
            }
            if let Some(dirs) = autoload.get("classmap") {
                let paths = strings(dirs)
                    .into_iter()
                    .filter_map(|p| self.safe(base, p))
                    .collect::<Vec<_>>();
                self.classmap_roots.extend(paths.clone());
                self.roots.extend(paths);
            }
            if let Some(files) = autoload.get("files") {
                let paths = strings(files)
                    .into_iter()
                    .filter_map(|p| self.safe(base, p))
                    .collect::<Vec<_>>();
                self.files.extend(paths);
            }
            if let Some(excludes) = autoload.get("exclude-from-classmap") {
                self.excludes.extend(strings(excludes).into_iter().map(|s| {
                    (base.to_owned(), s.trim_start_matches('/').to_owned())
                }));
            }
        }
    }
    fn read(&mut self, path: &Path, hash: &mut Sha256) -> Option<Value> {
        let base = path.parent()?;
        let name = path.file_name()?.to_str()?;
        let path = self.safe(base, name)?;
        self.manifests.push(path.to_owned());
        hash.update(path.to_string_lossy().as_bytes());
        match bounded_json(&path) {
            Ok((v, b)) => {
                hash.update(&b);
                Some(v)
            }
            Err(e) => {
                hash.update(b"missing-or-invalid");
                if e.kind() != io::ErrorKind::NotFound {
                    self.warnings.push(format!("{}: {e}", path.display()));
                }
                None
            }
        }
    }
    pub fn load(root: &Path) -> Self {
        let canonical = root.canonicalize().unwrap_or_else(|_| root.to_owned());
        let root = canonical.as_path();
        let mut this = Self {
            workspace: canonical.clone(),
            ..Default::default()
        };
        let mut hash = Sha256::new();
        let manifest = this
            .read(&root.join("composer.json"), &mut hash)
            .unwrap_or(Value::Null);
        this.autoload(root, &manifest, true);
        let Some(vendor) = this.safe(
            root,
            manifest
                .pointer("/config/vendor-dir")
                .and_then(Value::as_str)
                .unwrap_or("vendor"),
        ) else {
            this.compiled_excludes = this
                .excludes
                .iter()
                .filter_map(|(base, pattern)| {
                    let pattern = if pattern.ends_with('/') {
                        format!("{pattern}**")
                    } else {
                        pattern.clone()
                    };
                    globset::Glob::new(&pattern)
                        .ok()
                        .map(|g| (base.clone(), g.compile_matcher()))
                })
                .collect();
            this.fingerprint = format!("{:x}", hash.finalize());
            return this;
        };
        this.vendor = vendor.clone();
        // Lock participates in cache invalidation. Missing packages are not invented.
        this.read(&root.join("composer.lock"), &mut hash);
        if let Some(installed) =
            this.read(&vendor.join("composer/installed.json"), &mut hash)
        {
            let packages =
                installed.get("packages").unwrap_or(&installed).as_array();
            if let Some(packages) = packages {
                for package in packages {
                    let Some(name) = package.get("name").and_then(Value::as_str)
                    else {
                        continue;
                    };
                    this.packages.push(name.to_owned());
                    let base = if let Some(p) =
                        package.get("install-path").and_then(Value::as_str)
                    {
                        this.safe(&vendor.join("composer"), p)
                    } else {
                        this.safe(&vendor, name)
                    };
                    let Some(base) = base else { continue };
                    if let Some(json) =
                        this.read(&base.join("composer.json"), &mut hash)
                    {
                        this.autoload(&base, &json, false);
                    } else {
                        this.autoload(&base, package, false);
                    }
                }
            }
        } else if let Ok(entries) = std::fs::read_dir(&vendor) {
            // Composer 1/no installed.json: bounded depth vendor/vendor/package manifests.
            for scope in entries.flatten().take(4096) {
                let Some(scope_path) =
                    this.safe(&vendor, &scope.file_name().to_string_lossy())
                else {
                    continue;
                };
                if let Ok(packages) = std::fs::read_dir(&scope_path) {
                    for package in packages.flatten().take(4096) {
                        let Some(base) = this.safe(
                            &scope_path,
                            &package.file_name().to_string_lossy(),
                        ) else {
                            continue;
                        };
                        let path = base.join("composer.json");
                        if path.is_file() {
                            if let Some(json) = this.read(&path, &mut hash) {
                                if let Some(name) =
                                    json.get("name").and_then(Value::as_str)
                                {
                                    this.packages.push(name.to_owned());
                                }
                                this.autoload(&base, &json, false);
                            }
                        }
                    }
                }
            }
        }
        this.compiled_excludes = this
            .excludes
            .iter()
            .filter_map(|(base, pattern)| {
                let pattern = if pattern.ends_with('/') {
                    format!("{pattern}**")
                } else {
                    pattern.clone()
                };
                globset::Glob::new(&pattern)
                    .ok()
                    .map(|g| (base.clone(), g.compile_matcher()))
            })
            .collect();
        this.fingerprint = format!("{:x}", hash.finalize());
        this.roots.sort();
        this.roots.dedup();
        this.files.sort();
        this.files.dedup();
        this
    }
    pub fn excluded(&self, path: &Path) -> bool {
        let is_classmap = self.classmap_roots.iter().any(|p| path.starts_with(p));
        let is_psr = self
            .roots
            .iter()
            .filter(|p| !self.classmap_roots.contains(p))
            .any(|p| path.starts_with(p));
        if !is_classmap || is_psr || self.files.iter().any(|p| p == path) {
            return false;
        }
        self.compiled_excludes.iter().any(|(base, matcher)| {
            path.strip_prefix(base)
                .is_ok_and(|relative| matcher.is_match(relative))
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mappings_longest_arrays_dev_fallback_files_and_vendor() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::create_dir_all(root.join("vendor/acme/pkg")).unwrap();
        std::fs::write(root.join("composer.json"),r#"{"autoload":{"psr-4":{"App\\":["src","generated"],"App\\Models\\":"models","":"fallback"},"classmap":["legacy"],"files":["helpers.php"],"exclude-from-classmap":["/tests/"]},"autoload-dev":{"psr-4":{"Tests\\":"tests"}}}"#).unwrap();
        std::fs::write(
            root.join("vendor/acme/pkg/composer.json"),
            r#"{"name":"acme/pkg","autoload":{"psr-4":{"Acme\\":"lib"}}}"#,
        )
        .unwrap();
        let c = Composer::load(root);
        let p = c.resolve("App\\Models\\User");
        assert_eq!(p[0], root.join("models/User.php"));
        assert_eq!(p[1], root.join("src/Models/User.php"));
        assert_eq!(p[2], root.join("generated/Models/User.php"));
        assert_eq!(p[3], root.join("fallback/App/Models/User.php"));
        assert_eq!(c.resolve("Tests\\Unit")[0], root.join("tests/Unit.php"));
        assert!(c.roots.contains(&root.join("legacy")));
        assert_eq!(c.files, vec![root.join("helpers.php")]);
        assert!(!c.excluded(&root.join("tests/a.php")));
        assert_eq!(
            c.resolve("Acme\\Thing")[0],
            root.join("vendor/acme/pkg/lib/Thing.php")
        );
        let old = c.fingerprint;
        std::fs::write(root.join("composer.lock"), "{}").unwrap();
        assert_ne!(old, Composer::load(root).fingerprint);
    }
    #[test]
    fn classmap_exclusions_do_not_hide_psr4_and_internal_vendor_links_work() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        std::fs::create_dir_all(r.join("legacy/tests")).unwrap();
        std::fs::create_dir_all(r.join("tests")).unwrap();
        std::fs::write(r.join("composer.json"),r#"{"autoload":{"classmap":["legacy"],"exclude-from-classmap":["/legacy/tests/","/tests/"]},"autoload-dev":{"psr-4":{"Tests\\":"tests"}}}"#).unwrap();
        let c = Composer::load(r);
        assert!(c.excluded(&r.join("legacy/tests/A.php")));
        assert!(!c.excluded(&r.join("tests/A.php")));
        #[cfg(unix)]
        {
            std::fs::create_dir_all(r.join("packages/pkg/src")).unwrap();
            std::fs::create_dir_all(r.join("vendor/acme")).unwrap();
            std::fs::write(
                r.join("packages/pkg/composer.json"),
                r#"{"autoload":{"psr-4":{"Inside\\":"src"}}}"#,
            )
            .unwrap();
            std::os::unix::fs::symlink(
                r.join("packages/pkg"),
                r.join("vendor/acme/pkg"),
            )
            .unwrap();
            let c = Composer::load(r);
            assert_eq!(c.resolve("Inside\\A")[0], r.join("packages/pkg/src/A.php"));
        }
    }
    #[test]
    fn declared_paths_cannot_escape_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(root.join("composer.json"),r#"{"autoload":{"psr-4":{"Bad\\":"../outside","Abs\\":"/etc"},"classmap":["../outside"],"files":["../outside/secret.php"]}}"#).unwrap();
        let c = Composer::load(&root);
        assert!(c.roots.is_empty());
        assert!(c.files.is_empty());
        assert!(c.warnings.len() >= 4);
        assert!(c.resolve("Bad\\Secret").is_empty());
        #[cfg(unix)]
        {
            std::fs::create_dir_all(root.join("vendor/acme")).unwrap();
            std::os::unix::fs::symlink(&outside, root.join("vendor/acme/pkg"))
                .unwrap();
            std::fs::write(
                outside.join("composer.json"),
                r#"{"autoload":{"psr-4":{"Outside\\":"src"}}}"#,
            )
            .unwrap();
            let c = Composer::load(&root);
            assert!(c.resolve("Outside\\Secret").is_empty());
            assert!(!c.warnings.is_empty());
        }
    }
    #[test]
    fn installed_custom_vendor_and_malformed_safe() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        std::fs::create_dir_all(r.join("deps/composer")).unwrap();
        std::fs::create_dir_all(r.join("deps/a/b")).unwrap();
        std::fs::write(
            r.join("composer.json"),
            r#"{"config":{"vendor-dir":"deps"}}"#,
        )
        .unwrap();
        std::fs::write(r.join("deps/composer/installed.json"),r#"{"packages":[{"name":"a/b","install-path":"../a/b","autoload":{"psr-4":{"A\\":"src"}}}]}"#).unwrap();
        let c = Composer::load(r);
        assert!(c.resolve("A\\B")[0].ends_with("deps/a/b/src/B.php"));
        std::fs::write(r.join("composer.json"), "{").unwrap();
        assert!(!Composer::load(r).warnings.is_empty());
    }
}
