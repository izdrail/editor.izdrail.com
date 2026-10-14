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


/// Framework commands offered for every Laravel project, in display order.
/// `make:*` commands prompt for their name in the terminal.
const ARTISAN_BUILTIN: &[&str] = &[
    "serve", "test", "tinker", "migrate", "migrate:fresh", "migrate:rollback",
    "migrate:status", "db:seed", "route:list", "queue:work", "schedule:work",
    "optimize:clear", "cache:clear", "config:clear", "view:clear", "key:generate",
    "storage:link", "make:model", "make:controller", "make:migration",
    "make:request", "make:middleware", "make:seeder", "make:factory",
    "make:command", "make:job", "make:event", "make:listener",
];

/// The command name from a Laravel signature such as `mail:send {user} {--queue}`.
fn signature_name(sig: &str) -> Option<String> {
    let name = sig.split_whitespace().next()?;
    let ok = !name.is_empty()
        && name.len() <= 100
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '_' | '.'));
    ok.then(|| name.to_string())
}

/// Names declared by `$signature = '...'` in a PHP file.
fn signatures_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("$signature") {
        rest = &rest[i + "$signature".len()..];
        let after = rest.trim_start();
        let Some(after) = after.strip_prefix('=') else { continue };
        let after = after.trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '\'' || *c == '"') else {
            continue;
        };
        let body = &after[1..];
        if let Some(end) = body.find(q) {
            if let Some(name) = signature_name(&body[..end]) {
                out.push(name);
            }
        }
    }
    out
}

/// Names from `Artisan::command('name {arg}', ...)` closures (routes/console.php).
fn closure_commands_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("Artisan::command(") {
        rest = &rest[i + "Artisan::command(".len()..];
        let arg = rest.trim_start();
        let Some(q) = arg.chars().next().filter(|c| *c == '\'' || *c == '"') else {
            continue;
        };
        let body = &arg[1..];
        if let Some(end) = body.find(q) {
            if let Some(name) = signature_name(&body[..end]) {
                out.push(name);
            }
        }
    }
    out
}

/// Built-in commands first, then the project's own, without duplicates.
/// Reads `app/Console/Commands/*.php` and `routes/console.php` only; PHP is
/// never executed, so package-provided commands are not listed.
fn artisan_commands(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = ARTISAN_BUILTIN.iter().map(|s| s.to_string()).collect();
    let mut custom = Vec::new();
    let read = |path: &Path| -> Option<String> {
        let meta = std::fs::metadata(path).ok()?;
        (meta.is_file() && meta.len() <= MAX_FILE_BYTES)
            .then(|| std::fs::read_to_string(path).ok())
            .flatten()
    };
    if let Ok(dir) = std::fs::read_dir(root.join("app").join("Console").join("Commands")) {
        let mut files: Vec<_> = dir
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "php"))
            .collect();
        files.sort();
        for f in files.into_iter().take(MAX_SCRIPTS) {
            if let Some(text) = read(&f) {
                custom.extend(signatures_in(&text));
            }
        }
    }
    if let Some(text) = read(&root.join("routes").join("console.php")) {
        custom.extend(closure_commands_in(&text));
    }
    for name in custom {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// Run configurations for the project at `root`, in a stable order.
pub fn discover(root: &Path) -> Vec<RunDebugConfig> {
    let mut out = Vec::new();

    if root.join("artisan").is_file() {
        for name in artisan_commands(root) {
            out.push(config(
                format!("artisan: {name}"),
                "php",
                &["artisan", &name],
            ));
        }
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

    #[test]
    fn artisan_lists_builtin_and_project_commands() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "artisan", "<?php");
        let cmds = dir.path().join("app/Console/Commands");
        std::fs::create_dir_all(&cmds).unwrap();
        std::fs::write(
            cmds.join("SendEmails.php"),
            "<?php\nclass SendEmails {\n    protected $signature = 'mail:send {user} {--queue}';\n}\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("routes")).unwrap();
        std::fs::write(
            dir.path().join("routes/console.php"),
            "<?php\nArtisan::command(\"inspire:me {n}\", function () {});\nArtisan::command('migrate', fn() => 1);\n",
        )
        .unwrap();
        let c = discover(dir.path());
        let names: Vec<_> = c.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"artisan: serve"));
        assert!(names.contains(&"artisan: make:model"));
        assert!(names.contains(&"artisan: mail:send"));
        assert!(names.contains(&"artisan: inspire:me"));
        assert_eq!(names.iter().filter(|n| **n == "artisan: migrate").count(), 1);
        let send = c.iter().find(|c| c.name == "artisan: mail:send").unwrap();
        assert_eq!(send.program, "php");
        assert_eq!(send.args.as_deref().unwrap(), ["artisan", "mail:send"]);
    }

    #[test]
    fn odd_signatures_are_ignored() {
        assert_eq!(signatures_in("$signature = $x;"), Vec::<String>::new());
        assert_eq!(signatures_in("$signature='a b'"), vec!["a".to_string()]);
        assert!(signature_name("").is_none());
        assert!(signature_name("bad;name x").is_none());
    }
}
