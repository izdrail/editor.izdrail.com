//! Run configurations discovered from a web project's own files, so the
//! "Run and Debug" palette lists `npm run dev`, `composer run-script test` and
//! `php artisan serve` without anyone writing `.lapce/run.toml`.
//!
//! Only `package.json`, `composer.json`, lockfiles, `artisan` and
//! `bin/console` in the workspace root are looked at (size bounded). Nothing
//! is executed.
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

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


/// Composer commands that make sense in any project with a `composer.json`.
const COMPOSER_BUILTIN: &[&str] = &["install", "update", "dump-autoload", "outdated", "audit"];

/// Symfony console commands offered when `bin/console` exists. `make:*` needs
/// symfony/maker-bundle and prints an error otherwise.
const SYMFONY_BUILTIN: &[&str] = &[
    "cache:clear", "debug:router", "debug:container", "debug:config", "lint:twig",
    "lint:yaml", "doctrine:migrations:migrate", "doctrine:migrations:diff",
    "doctrine:schema:validate", "messenger:consume", "make:controller", "make:entity",
    "make:migration",
];

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
    for name in refreshed_artisan(root) {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

const REFRESH_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_REFRESH_BYTES: u64 = 4 * 1024 * 1024;
const MAX_REFRESHED: usize = 600;

fn refreshed() -> &'static Mutex<HashMap<PathBuf, Vec<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Vec<String>>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Commands loaded by an explicit refresh in this session (empty before one).
fn refreshed_artisan(root: &Path) -> Vec<String> {
    refreshed()
        .lock()
        .ok()
        .and_then(|c| c.get(root).cloned())
        .unwrap_or_default()
}

/// Command names from `php artisan list --format=json`.
fn parse_artisan_list(json: &str) -> Vec<String> {
    let Ok(doc) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    doc.get("commands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.get("name").and_then(Value::as_str))
        .filter(|n| !matches!(*n, "help" | "list" | "_complete" | "completion"))
        .filter_map(signature_name)
        .take(MAX_REFRESHED)
        .collect()
}

/// Runs `<php> artisan list --format=json` in `root` and caches the command
/// names for this session. This EXECUTES the project's PHP (service providers
/// boot), so it is only called from an explicit user command, never from
/// palette discovery. Returns the number of commands found.
pub fn refresh_artisan(root: &Path, php: &str) -> Result<usize, String> {
    if !root.join("artisan").is_file() {
        return Err("no artisan file in the workspace root".into());
    }
    let mut child = Command::new(php)
        .args(["artisan", "list", "--format=json", "--no-interaction"])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start {php}: {e}"))?;
    let mut stdout = child.stdout.take().ok_or("no output pipe")?;
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _ = (&mut stdout).take(MAX_REFRESH_BYTES + 1).read_to_string(&mut out);
        out
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() < REFRESH_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("php artisan list timed out after 20 seconds".into());
            }
        }
    };
    let out = reader.join().unwrap_or_default();
    if !status.success() {
        return Err(format!("php artisan list failed ({status})"));
    }
    if out.len() as u64 > MAX_REFRESH_BYTES {
        return Err("php artisan list produced too much output".into());
    }
    let names = parse_artisan_list(&out);
    if names.is_empty() {
        return Err("php artisan list returned no commands".into());
    }
    let n = names.len();
    if let Ok(mut c) = refreshed().lock() {
        c.insert(root.to_path_buf(), names);
    }
    Ok(n)
}

/// Run configurations for the project at `root`, in a stable order.
/// Targets of a `Makefile`: `name:` lines, skipping special (`.PHONY`), pattern
/// (`%.o`), variable-assignment (`A := 1`) and indented recipe lines.
fn make_targets(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.starts_with(['\t', ' ', '#', '.']) || line.is_empty() {
            continue;
        }
        let Some((head, tail)) = line.split_once(':') else { continue };
        // `VAR := x`, `VAR ::= x`, `VAR:=x`; a rule's `:` is followed by a prerequisite or end.
        if tail.starts_with('=') || tail.starts_with(":=") || head.contains(['=', '$', '%', '(']) {
            continue;
        }
        for t in head.split_whitespace() {
            let ok = !t.is_empty()
                && t.len() <= 60
                && t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
                && !t.starts_with(['-', '/']);
            if ok && !out.iter().any(|o| o == t) && out.len() < MAX_SCRIPTS {
                out.push(t.to_string());
            }
        }
    }
    out
}

fn makefile(root: &Path) -> Option<String> {
    for name in ["Makefile", "makefile", "GNUmakefile"] {
        let p = root.join(name);
        let Ok(meta) = std::fs::symlink_metadata(&p) else { continue };
        if meta.file_type().is_file() && meta.len() <= MAX_FILE_BYTES {
            return std::fs::read_to_string(&p).ok();
        }
    }
    None
}

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
        for name in SYMFONY_BUILTIN {
            out.push(config(format!("symfony: {name}"), "php", &["bin/console", name]));
        }
    }

    if let Some(doc) = read_json(&root.join("composer.json")) {
        for name in COMPOSER_BUILTIN {
            // A script of the same name wins and is listed below.
            if !script_names(&doc).iter().any(|s| s == name) {
                out.push(config(format!("composer: {name}"), "composer", &[name]));
            }
        }
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

    // Laravel Sail, when its wrapper script is installed.
    let sail = root.join("vendor").join("bin").join("sail");
    if std::fs::symlink_metadata(&sail).is_ok_and(|m| m.file_type().is_file()) {
        let sail = sail.to_string_lossy().into_owned();
        for (label, args) in [
            ("up", &["up"][..]),
            ("up -d", &["up", "-d"]),
            ("down", &["down"]),
            ("test", &["test"]),
            ("shell", &["shell"]),
        ] {
            out.push(config(format!("sail: {label}"), &sail, args));
        }
    }
    // Docker Compose, when a compose file is present.
    if ["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"]
        .iter()
        .any(|f| root.join(f).is_file())
    {
        for (label, args) in [
            ("up", &["compose", "up"][..]),
            ("up -d", &["compose", "up", "-d"]),
            ("down", &["compose", "down"]),
            ("logs -f", &["compose", "logs", "-f"]),
            ("ps", &["compose", "ps"]),
        ] {
            out.push(config(format!("docker: {label}"), "docker", args));
        }
    }
    // DDEV and Lando local environments (common for WordPress and Drupal).
    if root.join(".ddev").join("config.yaml").is_file() {
        for (label, args) in [
            ("start", &["start"][..]),
            ("stop", &["stop"]),
            ("restart", &["restart"]),
            ("launch", &["launch"]),
            ("describe", &["describe"]),
            ("logs", &["logs", "-f"]),
        ] {
            out.push(config(format!("ddev: {label}"), "ddev", args));
        }
    }
    if root.join(".lando.yml").is_file() || root.join(".lando.yaml").is_file() {
        for (label, args) in [
            ("start", &["start"][..]),
            ("stop", &["stop"]),
            ("rebuild", &["rebuild", "-y"]),
            ("info", &["info"]),
            ("logs", &["logs", "-f"]),
        ] {
            out.push(config(format!("lando: {label}"), "lando", args));
        }
    }
    if let Some(text) = makefile(root) {
        for t in make_targets(&text) {
            out.push(config(format!("make: {t}"), "make", &[&t]));
        }
    }
    // Agent CLI tools (Claude Code, Codex CLI, opencode) found on this
    // machine, each by its full path so a terminal with a minimal PATH
    // still launches them.
    for (tool, path) in agent_cli_tools() {
        let program = path.to_string_lossy().into_owned();
        out.push(config(format!("agent: {tool}"), &program, &[]));
    }
    out
}

/// Known agent CLI binaries, in the order they are listed.
pub const AGENT_CLI_NAMES: &[&str] = &["claude", "codex", "opencode"];

/// Directories searched for agent CLIs: $PATH (first 64 entries), the usual
/// user install spots under $HOME, and the common system prefixes.
fn agent_cli_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).take(64).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        for extra in [
            ".local/bin",
            ".npm-global/bin",
            ".claude/local",
            ".bun/bin",
            ".cargo/bin",
            "bin",
        ] {
            dirs.push(home.join(extra));
        }
    }
    for dir in ["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin"] {
        dirs.push(PathBuf::from(dir));
    }
    dirs
}

fn find_tool_in(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    dirs.iter().map(|d| d.join(name)).find(|p| p.is_file())
}

/// The installed agent CLIs as `(name, full path)`, filesystem metadata
/// only - nothing is executed to discover them.
pub fn agent_cli_tools() -> Vec<(&'static str, PathBuf)> {
    let dirs = agent_cli_dirs();
    AGENT_CLI_NAMES
        .iter()
        .copied()
        .filter_map(|t| find_tool_in(&dirs, t).map(|p| (t, p)))
        .collect()
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
        assert!(discover(dir.path()).iter().all(|c| c.name.starts_with("agent: ")));
    }

    #[test]
    fn finds_agent_clis_by_metadata_only() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        write(&a, "claude", "#!/bin/sh\n");
        write(&b, "opencode", "#!/bin/sh\n");
        write(&b, "not-a-tool", "x");
        let dirs = vec![a.clone(), b.clone()];
        assert_eq!(find_tool_in(&dirs, "claude"), Some(a.join("claude")));
        assert_eq!(find_tool_in(&dirs, "codex"), None);
        assert_eq!(find_tool_in(&dirs, "opencode"), Some(b.join("opencode")));
        assert_eq!(find_tool_in(&dirs, "not-a-tool"), Some(b.join("not-a-tool")));
        // Missing directories are skipped, not an error.
        let dirs = vec![dir.path().join("nope"), a.clone()];
        assert_eq!(find_tool_in(&dirs, "claude"), Some(a.join("claude")));
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
        assert!(discover(dir.path()).iter().all(|c| c.name.starts_with("agent: ")));
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
    fn composer_and_symfony_builtins() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "composer.json", r#"{"scripts":{"audit":"x","test":"phpunit"}}"#);
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        write(&dir.path().join("bin"), "console", "<?php");
        let c = discover(dir.path());
        let get = |n: &str| c.iter().filter(|c| c.name == n).count();
        assert_eq!(get("composer: install"), 1);
        assert_eq!(get("composer: dump-autoload"), 1);
        // The project's own `audit` script replaces the built-in, not duplicates it.
        assert_eq!(get("composer: audit"), 1);
        let audit = c.iter().find(|c| c.name == "composer: audit").unwrap();
        assert_eq!(audit.args.as_deref().unwrap(), ["run-script", "audit"]);
        assert_eq!(get("symfony: cache:clear"), 1);
        let cc = c.iter().find(|c| c.name == "symfony: debug:router").unwrap();
        assert_eq!(cc.args.as_deref().unwrap(), ["bin/console", "debug:router"]);
    }

    #[test]
    fn odd_signatures_are_ignored() {
        assert_eq!(signatures_in("$signature = $x;"), Vec::<String>::new());
        assert_eq!(signatures_in("$signature='a b'"), vec!["a".to_string()]);
        assert!(signature_name("").is_none());
        assert!(signature_name("bad;name x").is_none());
    }

    #[test]
    fn parses_artisan_list_json() {
        let json = r#"{"application":{"name":"Laravel"},"commands":[
            {"name":"about"},{"name":"help"},{"name":"list"},{"name":"_complete"},
            {"name":"horizon:status"},{"name":"bad name;x"}],"namespaces":[]}"#;
        assert_eq!(parse_artisan_list(json), vec!["about", "horizon:status", "bad"]);
        assert!(parse_artisan_list("not json").is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn refresh_runs_the_given_php_and_merges_into_the_list() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "artisan", "<?php");
        let fake = dir.path().join("fakephp");
        std::fs::write(&fake, "#!/bin/sh\n[ \"$1\" = artisan ] && [ \"$2\" = list ] || exit 3\necho '{\"commands\":[{\"name\":\"horizon:status\"},{\"name\":\"serve\"}]}'\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Before a refresh, package commands are not listed and nothing runs.
        let before: Vec<_> = discover(dir.path()).into_iter().map(|c| c.name).collect();
        assert!(!before.contains(&"artisan: horizon:status".to_string()));
        assert_eq!(refresh_artisan(dir.path(), fake.to_str().unwrap()), Ok(2));
        let after: Vec<_> = discover(dir.path()).into_iter().map(|c| c.name).collect();
        assert!(after.contains(&"artisan: horizon:status".to_string()));
        assert_eq!(after.iter().filter(|n| *n == "artisan: serve").count(), 1);
        // Failures leave the cache alone and report an error.
        let bad = dir.path().join("badphp");
        std::fs::write(&bad, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(refresh_artisan(dir.path(), bad.to_str().unwrap()).is_err());
        assert!(refresh_artisan(dir.path(), "/nonexistent/php").is_err());
        let other = tempfile::tempdir().unwrap();
        assert!(refresh_artisan(other.path(), "php").is_err());
        assert!(discover(dir.path()).iter().any(|c| c.name == "artisan: horizon:status"));
    }

    #[test]
    fn makefile_targets_are_listed() {
        let t = "CC := gcc\nVERSION ?= 1\n.PHONY: test\n\nall: build test\n\tcc x\n\nbuild test:\n\techo\n%.o: %.c\n\tcc\nup-dev: ; docker compose up\nbad target$(X):\n# comment: x\nlint:\n  indented:\nlint:\n";
        assert_eq!(make_targets(t), ["all", "build", "test", "up-dev", "lint"]);
        assert!(make_targets("").is_empty());
        assert_eq!(make_targets("a:b\n").len(), 1);
        let many: String = (0..500).map(|i| format!("t{i}:\n")).collect();
        assert_eq!(make_targets(&many).len(), MAX_SCRIPTS);
    }

    #[test]
    fn sail_docker_and_make_configs() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        write(r, "Makefile", "deploy:\n\techo\n");
        write(r, "compose.yaml", "services: {}\n");
        std::fs::create_dir_all(r.join("vendor/bin")).unwrap();
        write(r, "vendor/bin/sail", "#!/bin/sh\n");
        let names: Vec<String> = discover(r).into_iter().map(|c| c.name).collect();
        for n in ["sail: up -d", "sail: test", "docker: up", "docker: logs -f", "make: deploy"] {
            assert!(names.iter().any(|x| x == n), "{n} in {names:?}");
        }
        let sail = discover(r).into_iter().find(|c| c.name == "sail: up -d").unwrap();
        assert!(sail.program.ends_with("vendor/bin/sail"));
        assert_eq!(sail.args, Some(vec!["up".to_string(), "-d".to_string()]));
        let make = discover(r).into_iter().find(|c| c.name == "make: deploy").unwrap();
        assert_eq!((make.program.as_str(), make.args), ("make", Some(vec!["deploy".to_string()])));
        // None of the three without their files.
        let e = tempfile::tempdir().unwrap();
        assert!(discover(e.path()).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_or_huge_makefile_is_ignored() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        write(r, "real.mk", "evil:\n");
        std::os::unix::fs::symlink(r.join("real.mk"), r.join("Makefile")).unwrap();
        assert!(makefile(r).is_none());
        std::fs::remove_file(r.join("Makefile")).unwrap();
        write(r, "Makefile", &"# pad\n".repeat(300_000));
        assert!(makefile(r).is_none());
    }

    #[test]
    fn ddev_and_lando_entries() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        assert!(discover(r).is_empty());
        std::fs::create_dir_all(r.join(".ddev")).unwrap();
        write(r, ".ddev/config.yaml", "name: x\n");
        write(r, ".lando.yml", "name: x\n");
        let all = discover(r);
        let find = |n: &str| all.iter().find(|c| c.name == n).unwrap().clone();
        assert_eq!(find("ddev: start").program, "ddev");
        assert_eq!(find("ddev: logs").args, Some(vec!["logs".to_string(), "-f".to_string()]));
        assert_eq!(find("lando: rebuild").args, Some(vec!["rebuild".to_string(), "-y".to_string()]));
        assert_eq!(all.len(), 11);
    }
}
