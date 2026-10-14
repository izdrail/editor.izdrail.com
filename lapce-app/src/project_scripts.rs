//! Run configurations discovered from a web project's own files, so the
//! "Run and Debug" palette lists `npm run dev`, `composer run-script test` and
//! `php artisan serve` without anyone writing `.lapce/run.toml`.
//!
//! Only `package.json`, `composer.json`, lockfiles, `artisan` and
//! `bin/console` in the workspace root are looked at (size bounded). Nothing
//! is executed.
use std::path::Path;

use lapce_rpc::dap_types::{ConfigSource, DapId, RunDebugConfig};
use serde_json::Value;

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_SCRIPTS: usize = 200;

fn read_json(path: &Path) -> Option<Value> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn config(name: String, program: &str, args: &[&str]) -> RunDebugConfig {
    RunDebugConfig {
        ty: None,
        name,
        program: program.to_string(),
        args: Some(args.iter().map(|a| a.to_string()).collect()),
        cwd: Some("${workspace}".to_string()),
        env: None,
        prelaunch: None,
        debug_command: None,
        dap_id: DapId::default(),
        tracing_output: false,
        config_source: ConfigSource::Palette,
    }
}

/// The package manager implied by lockfiles; npm when none is present.
fn node_runner(root: &Path) -> &'static str {
    if root.join("pnpm-lock.yaml").is_file() {
        "pnpm"
    } else if root.join("yarn.lock").is_file() {
        "yarn"
    } else if root.join("bun.lockb").is_file() || root.join("bun.lock").is_file() {
        "bun"
    } else {
        "npm"
    }
}

/// npm runs `pre<name>`/`post<name>` itself and composer fires `post-*-cmd`
/// events itself, so listing them as runnable entries is noise. A hook only
/// counts when the script it wraps exists.
fn is_lifecycle_hook(name: &str, all: &[&String]) -> bool {
    if name.starts_with("post-") || name.starts_with("pre-") {
        return true;
    }
    for prefix in ["pre", "post"] {
        if let Some(base) = name.strip_prefix(prefix) {
            if !base.is_empty() && all.iter().any(|n| n.as_str() == base) {
                return true;
            }
        }
    }
    false
}

fn script_names(doc: &Value) -> Vec<String> {
    let Some(map) = doc.get("scripts").and_then(Value::as_object) else {
        return Vec::new();
    };
    let all: Vec<&String> = map.keys().collect();
    all.iter()
        .filter(|k| !k.is_empty() && !is_lifecycle_hook(k, &all))
        .take(MAX_SCRIPTS)
        .map(|k| (*k).clone())
        .collect()
}

/// Run configurations for the project at `root`, in a stable order.
pub fn discover(root: &Path) -> Vec<RunDebugConfig> {
    let mut out = Vec::new();

    if root.join("artisan").is_file() {
        out.push(config("artisan: serve".into(), "php", &["artisan", "serve"]));
        out.push(config("artisan: test".into(), "php", &["artisan", "test"]));
        out.push(config(
            "artisan: migrate".into(),
            "php",
            &["artisan", "migrate"],
        ));
    }
    if root.join("bin").join("console").is_file() {
        out.push(config("symfony: console".into(), "php", &["bin/console"]));
    }

    if let Some(doc) = read_json(&root.join("composer.json")) {
        for name in script_names(&doc) {
            out.push(config(
                format!("composer: {name}"),
                "composer",
                &["run-script", &name],
            ));
        }
    }

    if let Some(doc) = read_json(&root.join("package.json")) {
        let runner = node_runner(root);
        for name in script_names(&doc) {
            out.push(config(format!("{runner}: {name}"), runner, &["run", &name]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, text: &str) {
        std::fs::write(dir.join(name), text).unwrap();
    }

    #[test]
    fn empty_project_has_no_configs() {
        let dir = tempfile::tempdir().unwrap();
        assert!(discover(dir.path()).is_empty());
    }

    #[test]
    fn finds_npm_composer_and_artisan() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "artisan", "<?php");
        write(
            dir.path(),
            "composer.json",
            r#"{"scripts":{"test":"phpunit","lint":["phpcs"],"post-install-cmd":"x"}}"#,
        );
        write(
            dir.path(),
            "package.json",
            r#"{"scripts":{"dev":"vite","build":"vite build","prebuild":"x","postcss":"y","preview":"vite preview"}}"#,
        );
        let names: Vec<_> = discover(dir.path()).into_iter().map(|c| c.name).collect();
        assert!(names.contains(&"artisan: serve".to_string()));
        assert!(names.contains(&"composer: test".to_string()));
        assert!(names.contains(&"composer: lint".to_string()));
        assert!(!names.iter().any(|n| n.contains("post-install")));
        assert!(names.contains(&"npm: dev".to_string()));
        assert!(names.contains(&"npm: build".to_string()));
        assert!(!names.contains(&"npm: prebuild".to_string()));
        assert!(names.contains(&"npm: preview".to_string()));
        assert!(names.contains(&"npm: postcss".to_string()));
    }

    #[test]
    fn lockfile_picks_the_package_manager() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "package.json", r#"{"scripts":{"dev":"vite"}}"#);
        write(dir.path(), "pnpm-lock.yaml", "");
        let c = discover(dir.path());
        assert_eq!(c[0].program, "pnpm");
        assert_eq!(c[0].args.as_deref().unwrap(), ["run", "dev"]);
        assert_eq!(c[0].cwd.as_deref(), Some("${workspace}"));
    }

    #[test]
    fn bad_json_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "package.json", "{not json");
        assert!(discover(dir.path()).is_empty());
    }
}
