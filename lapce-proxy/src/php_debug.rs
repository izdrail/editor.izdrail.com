//! PHP / Xdebug debugging support: a managed install of the
//! `xdebug/vscode-php-debug` adapter, the adapter launch spec the generic DAP
//! client consumes, workspace detection, and Xdebug run configuration
//! templates.
//!
//! The adapter is a Node program (`out/phpDebug.js`) that speaks DAP on
//! stdin/stdout and listens for Xdebug on port 9003. It is installed the same
//! way as managed phpactor: pinned version, SHA-256 checked before anything is
//! unpacked or run, cached under the data directory.
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, ensure};
use lapce_core::directory::Directory;
use lapce_rpc::dap_types::{ConfigSource, DapId, DapServer, RunDebugConfig};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Pinned vscode-php-debug release. Bump all three constants together after
/// checking the digest on the GitHub release page.
pub const PHP_DEBUG_VERSION: &str = "1.40.2";
const PHP_DEBUG_URL: &str = "https://github.com/xdebug/vscode-php-debug/releases/download/v1.40.2/php-debug-1.40.2.vsix";
const PHP_DEBUG_SHA256: &str =
    "17631993fe800083a2fc89a6782c8bf58a79603ad201ddff60d9b59ba2345d5f";
/// Adapter entry point relative to the unpacked `extension/` directory
/// (`contributes.debuggers[0].program` in the extension manifest).
const ADAPTER_ENTRY: &str = "out/phpDebug.js";
/// The debug type used in `.lapce/run.toml` (`type = "php"`).
pub const PHP_DEBUG_TYPE: &str = "php";
/// Xdebug 3 default client port.
pub const DEFAULT_XDEBUG_PORT: u16 = 9003;

const MAX_DOWNLOAD: usize = 64 * 1024 * 1024;
const MAX_UNPACKED: u64 = 128 * 1024 * 1024;
const MAX_ENTRIES: usize = 5000;

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `<data dir>/debug-adapters/php-debug/<version>`.
pub fn adapter_dir() -> Option<PathBuf> {
    Some(
        Directory::data_local_directory()?
            .join("debug-adapters")
            .join("php-debug")
            .join(PHP_DEBUG_VERSION),
    )
}

fn entry_in(dir: &Path) -> PathBuf {
    dir.join("extension").join(ADAPTER_ENTRY)
}

/// Marker written after a fully verified unpack; the adapter is only used when
/// it exists, so a half-extracted directory is never run.
fn marker_in(dir: &Path) -> PathBuf {
    dir.join(".installed-sha256")
}

pub fn is_installed_in(dir: &Path) -> bool {
    entry_in(dir).is_file()
        && std::fs::read_to_string(marker_in(dir))
            .map(|s| s.trim() == PHP_DEBUG_SHA256)
            .unwrap_or(false)
}

pub fn is_installed() -> bool {
    adapter_dir().is_some_and(|d| is_installed_in(&d))
}

/// Unpack a verified vsix into `dest`, refusing paths that escape it.
fn unpack(bytes: &[u8], dest: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .context("php-debug package is not a valid archive")?;
    ensure!(zip.len() <= MAX_ENTRIES, "php-debug package has too many files");
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        // enclosed_name rejects absolute paths and `..` components.
        let Some(rel) = file.enclosed_name() else {
            anyhow::bail!("php-debug package contains an unsafe path");
        };
        if !rel.starts_with("extension") {
            continue;
        }
        let out = dest.join(&rel);
        if file.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        total += file.size();
        ensure!(total <= MAX_UNPACKED, "php-debug package is too large");
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut data = Vec::with_capacity(file.size() as usize);
        file.by_ref().take(MAX_UNPACKED).read_to_end(&mut data)?;
        std::fs::write(&out, data)?;
    }
    Ok(())
}

/// Install the pinned adapter into `dir` from already-downloaded bytes.
pub fn install_from_bytes(bytes: &[u8], dir: &Path) -> Result<()> {
    ensure!(bytes.len() < MAX_DOWNLOAD, "php-debug download too large");
    ensure!(
        sha256_hex(bytes) == PHP_DEBUG_SHA256,
        "php-debug download failed its SHA-256 check"
    );
    let parent = dir.parent().context("Invalid php-debug directory")?;
    std::fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new()
        .prefix(".php-debug-")
        .tempdir_in(parent)?;
    unpack(bytes, staging.path())?;
    ensure!(
        entry_in(staging.path()).is_file(),
        "php-debug package has no {ADAPTER_ENTRY}"
    );
    std::fs::write(marker_in(staging.path()), PHP_DEBUG_SHA256)?;
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    let staged = staging.keep();
    std::fs::rename(&staged, dir).inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&staged);
    })?;
    Ok(())
}

/// Return the adapter directory, downloading and verifying the pinned release
/// on first use. With `auto_install` false a missing adapter is an error.
pub fn ensure_adapter(auto_install: bool) -> Result<PathBuf> {
    let dir = adapter_dir().context("No data directory")?;
    if is_installed_in(&dir) {
        return Ok(dir);
    }
    ensure!(
        auto_install,
        "php-debug is not installed and auto-install is disabled"
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .user_agent("editor.izdrail.com")
        .build()?;
    let response = client.get(PHP_DEBUG_URL).send()?;
    ensure!(
        response.status().is_success(),
        "php-debug download failed: HTTP {}",
        response.status()
    );
    let bytes = response.bytes()?;
    install_from_bytes(&bytes, &dir)?;
    Ok(dir)
}

/// How to start the adapter. `program`/`args` are what the DAP client spawns;
/// `env` is extra environment for that process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterLaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

impl AdapterLaunchSpec {
    pub fn for_adapter_dir(dir: &Path, node: &str) -> Self {
        Self {
            program: node.to_string(),
            args: vec![entry_in(dir).to_string_lossy().into_owned()],
            env: HashMap::new(),
        }
    }

    pub fn into_dap_server(self, cwd: Option<PathBuf>) -> DapServer {
        DapServer {
            program: self.program,
            args: self.args,
            cwd,
        }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args).envs(&self.env);
        command
    }
}

fn command_exists(command: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    let exts: Vec<String> = if cfg!(windows) {
        vec![".exe".into(), ".cmd".into(), ".bat".into(), String::new()]
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&paths)
        .any(|d| exts.iter().any(|e| d.join(format!("{command}{e}")).is_file()))
}

/// Resolve the adapter launch spec, installing the adapter if needed.
/// Requires `node` on PATH (the adapter is a Node program); a missing node is
/// reported as an error the UI can show.
pub fn launch_spec(auto_install: bool) -> Result<AdapterLaunchSpec> {
    ensure!(
        command_exists("node"),
        "PHP debugging needs Node.js (`node`) on PATH to run the php-debug adapter"
    );
    let dir = ensure_adapter(auto_install)?;
    Ok(AdapterLaunchSpec::for_adapter_dir(&dir, "node"))
}

// ---------------------------------------------------------------- detection

const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Whether the workspace looks like a PHP project. Looks only at well known
/// files in the root (and one level of `public/`), never walks the tree and
/// executes nothing.
pub fn is_php_workspace(root: &Path) -> bool {
    if root.join("artisan").is_file()
        || root.join("bin/console").is_file()
        || root.join("wp-config.php").is_file()
        || root.join("wp-content").is_dir()
        || root.join("index.php").is_file()
        || root.join("public/index.php").is_file()
    {
        return true;
    }
    if let Ok(meta) = std::fs::metadata(root.join("composer.json")) {
        if meta.is_file() && meta.len() <= MAX_FILE_BYTES {
            return true;
        }
    }
    // A handful of loose .php files in the root.
    std::fs::read_dir(root).is_ok_and(|rd| {
        rd.take(500).flatten().any(|e| {
            e.path().extension().is_some_and(|x| x == "php") && e.path().is_file()
        })
    })
}

/// Project layout, used to choose the web server document root and the
/// default path mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Laravel,
    Symfony,
    WordPress,
    Plain,
}

pub fn detect_layout(root: &Path) -> Layout {
    if root.join("artisan").is_file() {
        Layout::Laravel
    } else if root.join("bin/console").is_file() {
        Layout::Symfony
    } else if root.join("wp-config.php").is_file() || root.join("wp-content").is_dir()
    {
        Layout::WordPress
    } else {
        Layout::Plain
    }
}

// --------------------------------------------------------------- templates

/// Server-side directories commonly used when PHP runs in a container or VM
/// and the code is mounted at the workspace root.
pub const COMMON_SERVER_ROOTS: &[&str] = &["/var/www/html", "/var/www", "/app"];

/// `pathMappings` for vscode-php-debug: server path -> local path. The
/// workspace is mapped from each common server root, most likely first for
/// the layout. The adapter ignores mappings that never match, so listing
/// several is harmless.
pub fn default_path_mappings(root: &Path, layout: Layout) -> Vec<(String, String)> {
    let local = root.to_string_lossy().into_owned();
    let mut roots: Vec<&str> = match layout {
        Layout::Laravel | Layout::Symfony => vec!["/var/www/html", "/app", "/var/www"],
        Layout::WordPress => vec!["/var/www/html", "/var/www"],
        Layout::Plain => vec!["/var/www/html", "/app", "/var/www"],
    };
    roots.dedup();
    roots
        .into_iter()
        .map(|s| (s.to_string(), local.clone()))
        .collect()
}

/// DAP `launch`/`attach` arguments for the adapter.
/// * `listen` (request `launch` without `program`): wait for Xdebug to connect
///   on `port` (the usual "Listen for Xdebug" flow with a web server).
/// * `script`: run `program` with the CLI PHP and debug it.
pub fn launch_arguments(
    root: &Path,
    layout: Layout,
    script: Option<&str>,
    port: u16,
) -> Value {
    let mappings: serde_json::Map<String, Value> =
        default_path_mappings(root, layout)
            .into_iter()
            .map(|(k, v)| (k, Value::String(v)))
            .collect();
    let mut args = json!({
        "name": if script.is_some() { "Launch PHP script" } else { "Listen for Xdebug" },
        "type": PHP_DEBUG_TYPE,
        "request": "launch",
        "port": port,
        "pathMappings": mappings,
        "cwd": root.to_string_lossy(),
    });
    if let Some(script) = script {
        args["program"] = Value::String(script.to_string());
    }
    args
}

fn php_config(name: &str, program: &str, args: &[&str], env: &[(&str, &str)]) -> RunDebugConfig {
    RunDebugConfig {
        ty: Some(PHP_DEBUG_TYPE.to_string()),
        name: name.to_string(),
        program: program.to_string(),
        args: Some(args.iter().map(|a| a.to_string()).collect()),
        cwd: Some("${workspace}".to_string()),
        env: if env.is_empty() {
            None
        } else {
            Some(env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
        },
        prelaunch: None,
        debug_command: None,
        dap_id: DapId::default(),
        tracing_output: false,
        config_source: ConfigSource::Palette,
    }
}

/// Xdebug run/debug configurations for a workspace; empty when it does not
/// look like PHP. Only generic `type = "php"` entries: they are offered in the
/// Run and Debug palette and a `.lapce/run.toml` entry of the same name wins.
pub fn xdebug_configs(root: &Path) -> Vec<RunDebugConfig> {
    if !is_php_workspace(root) {
        return Vec::new();
    }
    let port = DEFAULT_XDEBUG_PORT.to_string();
    let mut configs = vec![
        // Waits for Xdebug to connect (web requests). `program` is empty: the
        // client starts the adapter in listen mode.
        php_config("Listen for Xdebug", "", &[], &[("XDEBUG_PORT", &port)]),
        php_config(
            "Debug current PHP file",
            "php",
            &["${file}"],
            &[
                ("XDEBUG_MODE", "debug"),
                ("XDEBUG_CONFIG", "client_host=127.0.0.1 client_port=9003"),
            ],
        ),
    ];
    match detect_layout(root) {
        Layout::Laravel => configs.push(php_config(
            "Debug artisan serve",
            "php",
            &["artisan", "serve"],
            &[("XDEBUG_MODE", "debug"), ("XDEBUG_SESSION", "1")],
        )),
        Layout::Symfony => configs.push(php_config(
            "Debug bin/console",
            "php",
            &["bin/console"],
            &[("XDEBUG_MODE", "debug"), ("XDEBUG_SESSION", "1")],
        )),
        _ => {}
    }
    configs
}

/// Text for `.lapce/run.toml` with the Xdebug templates, to seed a new file.
pub fn run_toml_template(root: &Path) -> String {
    let layout = detect_layout(root);
    let mut out = String::from(
        "# Xdebug configurations (type = \"php\"). Listen on port 9003 and set\n\
         # pathMappings in your debug launch if PHP runs in Docker/a VM:\n",
    );
    for (server, local) in default_path_mappings(root, layout) {
        out.push_str(&format!("#   {server} -> {local}\n"));
    }
    out.push('\n');
    for c in xdebug_configs(root) {
        out.push_str(&config_toml(&c));
    }
    out
}

fn config_toml(c: &RunDebugConfig) -> String {
    let mut s = String::from("[[configs]]\n");
    s.push_str(&format!("name = {}\n", toml_str(&c.name)));
    if let Some(ty) = &c.ty {
        s.push_str(&format!("type = {}\n", toml_str(ty)));
    }
    s.push_str(&format!("program = {}\n", toml_str(&c.program)));
    if let Some(args) = &c.args {
        let items: Vec<String> = args.iter().map(|a| toml_str(a)).collect();
        s.push_str(&format!("args = [{}]\n", items.join(", ")));
    }
    if let Some(cwd) = &c.cwd {
        s.push_str(&format!("cwd = {}\n", toml_str(cwd)));
    }
    if let Some(env) = &c.env {
        let mut keys: Vec<_> = env.iter().collect();
        keys.sort();
        s.push_str("[configs.env]\n");
        for (k, v) in keys {
            s.push_str(&format!("{k} = {}\n", toml_str(v)));
        }
    }
    s.push('\n');
    s
}

fn toml_str(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}



/// Adapter-specific DAP arguments merged OVER the generic launch arguments.
/// Returns `None` (fail clearly, logged by the caller path) when the command
/// line cannot be mapped safely.
///
/// Script expansion: listen mode (empty `program`) omits `program` entirely -
/// never `program: ""`. When `program` is the PHP interpreter itself (php,
/// php.exe, php8.3, /usr/bin/php8.3, ...) the script is `args[0]`, the
/// remaining args replace the generic `args`, and `runtimeExecutable` carries
/// the interpreter so an explicit path like /usr/bin/php8.3 is honored over
/// PATH php. Interpreter flags before the script (`-d`, `-c`, ...) are
/// rejected instead of being misread as the script. Anything else is vscode
/// style: `program` IS the script and its args pass through untouched. The
/// generic keys `name`/`type`/`request` are owned by the generic arguments
/// and stripped here so the merge cannot clobber the session name.
fn php_extra_arguments(root: &Path, config: &RunDebugConfig) -> Option<Value> {
    let mut extra = launch_arguments(root, detect_layout(root), None, DEFAULT_XDEBUG_PORT);
    if let Some(map) = extra.as_object_mut() {
        map.remove("name");
        map.remove("type");
        map.remove("request");
    }
    if config.program.is_empty() {
        return Some(extra);
    }
    if !is_php_interpreter(&config.program) {
        extra["program"] = json!(config.program);
        return Some(extra);
    }
    let cli_args = config.args.clone().unwrap_or_default();
    match cli_args.first() {
        Some(first) if first.starts_with('-') => {
            tracing::warn!(
                target: "dap",
                "PHP debug config '{}': interpreter flags before the script ('{first}') are not supported",
                config.name
            );
            None
        }
        Some(script) => {
            extra["program"] = json!(script);
            extra["args"] = json!(cli_args[1..]);
            extra["runtimeExecutable"] = json!(config.program);
            Some(extra)
        }
        None => {
            tracing::warn!(
                target: "dap",
                "PHP debug config '{}' names the interpreter but no script",
                config.name
            );
            None
        }
    }
}

/// Whether `program` names the PHP interpreter itself (any path depth):
/// php, php.exe, php8, php8.3, /usr/bin/php8.3, C:\\php\\php.exe, ...
fn is_php_interpreter(program: &str) -> bool {
    let base = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    let base = base.strip_suffix(".exe").unwrap_or(&base);
    if base == "php" {
        return true;
    }
    base.strip_prefix("php").is_some_and(|rest| {
        !rest.is_empty()
            && rest.chars().any(|c| c.is_ascii_digit())
            && rest.chars().all(|c| c.is_ascii_digit() || c == '.')
    })
}

// --------------------------------------------------------- DAP integration

/// Build the PHP-aware adapter resolver installed in Dispatcher `Initialize`
/// (the workspace root is only known there, not in `Dispatcher::new`).
///
/// `type = "php"` configs resolve to the managed vscode-php-debug adapter,
/// installed on first use when `auto_install` is set; every other type falls
/// back to the generic `debug_command` resolution. PHP listen mode is a
/// `launch` request with `program` omitted (never `attach`, never
/// `program: ""`): the adapter waits for Xdebug to connect on
/// [`DEFAULT_XDEBUG_PORT`]. `config.env` stays the debuggee environment
/// (the generic launch arguments carry it); `AdapterSpec.env` is the adapter
/// process environment.
pub fn php_adapter_resolver(
    root: Option<PathBuf>,
    auto_install: bool,
) -> impl Fn(&RunDebugConfig) -> Option<crate::dap::AdapterSpec> + Send + Sync + 'static {
    move |config: &RunDebugConfig| {
        if config.ty.as_deref() != Some(PHP_DEBUG_TYPE) {
            return crate::dap::default_adapter(config);
        }
        let root = root
            .clone()
            .or_else(|| config.cwd.as_ref().map(PathBuf::from))
            .filter(|p| p.is_dir());
        let Some(root) = root else {
            tracing::warn!(
                target: "dap",
                "PHP debug config '{}' has no usable workspace root",
                config.name
            );
            return None;
        };
        match launch_spec(auto_install) {
            Ok(spec) => {
                let adapter_env = spec.env.clone();
                let server = spec.into_dap_server(None);
                Some(crate::dap::AdapterSpec {
                    env: adapter_env,
                    request: crate::dap::RequestKind::Launch,
                    extra_arguments: Some(php_extra_arguments(&root, config)?),
                    ..crate::dap::AdapterSpec::from_dap_server(
                        server,
                        PHP_DEBUG_TYPE.to_string(),
                    )
                })
            }
            Err(err) => {
                tracing::warn!(target: "dap", "PHP debug adapter unavailable: {err:#}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn make_vsix(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();
            for (name, data) in entries {
                zip.start_file(*name, opts).unwrap();
                zip.write_all(data).unwrap();
            }
            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn pin_is_well_formed() {
        assert_eq!(PHP_DEBUG_SHA256.len(), 64);
        assert!(PHP_DEBUG_SHA256.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(PHP_DEBUG_URL.contains(PHP_DEBUG_VERSION));
        assert!(PHP_DEBUG_URL.starts_with("https://github.com/xdebug/"));
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn install_rejects_wrong_hash_before_unpacking() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("php-debug").join("v");
        let vsix = make_vsix(&[("extension/out/phpDebug.js", b"x")]);
        let err = install_from_bytes(&vsix, &dir).unwrap_err().to_string();
        assert!(err.contains("SHA-256"), "{err}");
        assert!(!dir.exists());
    }

    #[test]
    fn unpack_extracts_extension_and_skips_rest() {
        let tmp = tempfile::tempdir().unwrap();
        let vsix = make_vsix(&[
            ("extension/out/phpDebug.js", b"js"),
            ("extension/package.json", b"{}"),
            ("[Content_Types].xml", b"<x/>"),
        ]);
        unpack(&vsix, tmp.path()).unwrap();
        assert!(entry_in(tmp.path()).is_file());
        assert!(!tmp.path().join("[Content_Types].xml").exists());
        assert!(!is_installed_in(tmp.path()), "no marker yet");
    }

    #[test]
    fn unpack_rejects_path_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let vsix = make_vsix(&[("extension/../../evil.js", b"x")]);
        assert!(unpack(&vsix, tmp.path()).is_err());
    }

    #[test]
    fn installed_requires_marker_with_pinned_hash() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(entry_in(tmp.path()).parent().unwrap()).unwrap();
        std::fs::write(entry_in(tmp.path()), "js").unwrap();
        assert!(!is_installed_in(tmp.path()));
        std::fs::write(marker_in(tmp.path()), "deadbeef").unwrap();
        assert!(!is_installed_in(tmp.path()));
        std::fs::write(marker_in(tmp.path()), PHP_DEBUG_SHA256).unwrap();
        assert!(is_installed_in(tmp.path()));
    }

    #[test]
    fn launch_spec_runs_node_on_entry() {
        let spec = AdapterLaunchSpec::for_adapter_dir(Path::new("/d/1.0"), "node");
        assert_eq!(spec.program, "node");
        assert_eq!(spec.args.len(), 1);
        assert!(spec.args[0].ends_with("extension/out/phpDebug.js"));
        let server = spec.into_dap_server(Some("/w".into()));
        assert_eq!(server.cwd, Some(PathBuf::from("/w")));
    }

    #[test]
    fn detects_php_workspaces_only() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_php_workspace(tmp.path()));
        assert!(xdebug_configs(tmp.path()).is_empty());
        std::fs::write(tmp.path().join("package.json"), "{}").unwrap();
        assert!(!is_php_workspace(tmp.path()));
        std::fs::write(tmp.path().join("composer.json"), "{}").unwrap();
        assert!(is_php_workspace(tmp.path()));
        assert_eq!(detect_layout(tmp.path()), Layout::Plain);
        std::fs::write(tmp.path().join("artisan"), "").unwrap();
        assert_eq!(detect_layout(tmp.path()), Layout::Laravel);
    }

    #[test]
    fn configs_are_php_typed_and_layout_aware() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("artisan"), "").unwrap();
        let configs = xdebug_configs(tmp.path());
        assert!(configs.iter().all(|c| c.ty.as_deref() == Some("php")));
        assert!(configs.iter().any(|c| c.name == "Listen for Xdebug"));
        assert!(configs.iter().any(|c| c.name == "Debug artisan serve"));
    }

    #[test]
    fn launch_arguments_listen_on_9003_with_mappings() {
        let args = launch_arguments(
            Path::new("/home/me/site"),
            Layout::Laravel,
            None,
            DEFAULT_XDEBUG_PORT,
        );
        assert_eq!(args["port"], 9003);
        assert_eq!(args["request"], "launch");
        assert!(args.get("program").is_none());
        assert_eq!(args["pathMappings"]["/var/www/html"], "/home/me/site");
        assert_eq!(args["pathMappings"]["/app"], "/home/me/site");
        let s = launch_arguments(Path::new("/w"), Layout::Plain, Some("a.php"), 9004);
        assert_eq!(s["program"], "a.php");
        assert_eq!(s["port"], 9004);
    }

    #[derive(Deserialize)]
    struct Parsed {
        configs: Vec<RunDebugConfig>,
    }

    #[test]
    fn run_toml_template_round_trips_through_the_run_config_parser() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("composer.json"), "{}").unwrap();
        std::fs::write(tmp.path().join("bin"), "").ok();
        let text = run_toml_template(tmp.path());
        let parsed: Parsed = toml::from_str(&text).unwrap();
        let names: Vec<_> = parsed.configs.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Listen for Xdebug", "Debug current PHP file"]);
        let file = &parsed.configs[1];
        assert_eq!(file.ty.as_deref(), Some("php"));
        assert_eq!(file.args.as_deref(), Some(&["${file}".to_string()][..]));
        assert_eq!(file.env.as_ref().unwrap()["XDEBUG_MODE"], "debug");
    }

    fn resolver_config(ty: &str, cwd: Option<&str>) -> RunDebugConfig {
        RunDebugConfig {
            ty: Some(ty.to_string()),
            name: format!("{ty} session"),
            program: String::new(),
            args: None,
            cwd: cwd.map(|c| c.to_string()),
            env: None,
            prelaunch: None,
            debug_command: None,
            dap_id: DapId::default(),
            tracing_output: false,
            config_source: ConfigSource::Palette,
        }
    }

    #[test]
    fn resolver_falls_back_to_debug_command_for_other_types() {
        let resolver = php_adapter_resolver(None, false);
        let mut config = resolver_config("node", None);
        assert!(resolver(&config).is_none());
        config.debug_command = Some(vec!["adapter".to_string(), "--stdio".to_string()]);
        let spec = resolver(&config).expect("debug_command should resolve");
        assert_eq!(spec.program, "adapter");
        assert_eq!(spec.adapter_id, "node");
    }

    #[test]
    fn resolver_reports_none_when_adapter_unavailable_without_install() {
        // auto_install = false and no verified adapter in the gate sandbox:
        // the PHP type must not panic or fall through to debug_command.
        let resolver = php_adapter_resolver(None, false);
        let config = resolver_config(PHP_DEBUG_TYPE, Some("/definitely/not/a/workspace"));
        assert!(resolver(&config).is_none());
    }


    #[test]
    fn extra_arguments_listen_omits_program_and_session_keys() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("artisan"), "").unwrap();
        let config = resolver_config(PHP_DEBUG_TYPE, None);
        let extra = php_extra_arguments(tmp.path(), &config).unwrap();
        assert!(extra.get("program").is_none());
        assert!(extra.get("name").is_none());
        assert!(extra.get("type").is_none());
        assert_eq!(extra["port"], 9003);
        assert_eq!(
            extra["pathMappings"]["/var/www/html"],
            tmp.path().to_string_lossy().as_ref()
        );
    }

    #[test]
    fn extra_arguments_expand_command_line_script_into_program() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = resolver_config(PHP_DEBUG_TYPE, None);
        config.program = "php".to_string();
        config.args = Some(vec!["artisan".to_string(), "serve".to_string()]);
        let extra = php_extra_arguments(tmp.path(), &config).unwrap();
        assert_eq!(extra["program"], "artisan");
        assert_eq!(extra["args"], serde_json::json!(["serve"]));
        assert_eq!(extra["runtimeExecutable"], "php");
    }

    #[test]
    fn extra_arguments_honor_explicit_interpreter_path() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = resolver_config(PHP_DEBUG_TYPE, None);
        config.program = "/usr/bin/php8.3".to_string();
        config.args = Some(vec!["bin/console".to_string()]);
        let extra = php_extra_arguments(tmp.path(), &config).unwrap();
        assert_eq!(extra["program"], "bin/console");
        assert_eq!(extra["runtimeExecutable"], "/usr/bin/php8.3");
    }

    #[test]
    fn extra_arguments_vscode_style_program_is_the_script() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = resolver_config(PHP_DEBUG_TYPE, None);
        config.program = "bin/console".to_string();
        let extra = php_extra_arguments(tmp.path(), &config).unwrap();
        assert_eq!(extra["program"], "bin/console");
        assert!(extra.get("args").is_none());
        assert!(extra.get("runtimeExecutable").is_none());
    }

    #[test]
    fn extra_arguments_vscode_style_keeps_script_args_unshifted() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = resolver_config(PHP_DEBUG_TYPE, None);
        config.program = "/w/index.php".to_string();
        config.args = Some(vec!["--verbose".to_string()]);
        let extra = php_extra_arguments(tmp.path(), &config).unwrap();
        assert_eq!(extra["program"], "/w/index.php");
        assert!(extra.get("args").is_none());
    }

    #[test]
    fn extra_arguments_reject_interpreter_flags_before_script() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = resolver_config(PHP_DEBUG_TYPE, None);
        config.program = "php".to_string();
        config.args = Some(vec![
            "-d".to_string(),
            "xdebug.mode=debug".to_string(),
            "a.php".to_string(),
        ]);
        assert!(php_extra_arguments(tmp.path(), &config).is_none());
    }

    #[test]
    fn php_interpreter_detection() {
        for yes in ["php", "php.exe", "PHP", "php8", "php8.3", "/usr/bin/php8.3", "php74"] {
            assert!(is_php_interpreter(yes), "{yes} should be an interpreter");
        }
        for no in ["index.php", "/w/index.php", "phpstorm", "php-cs-fixer", "bin/console", ""] {
            assert!(!is_php_interpreter(no), "{no} should not be an interpreter");
        }
    }

}
