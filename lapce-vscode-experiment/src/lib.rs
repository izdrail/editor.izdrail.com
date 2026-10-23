//! Opt-in, capability-free experiment. Not a VS Code extension host.
use rquickjs::{Context, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Component, Path},
    time::{Duration, Instant},
};

mod isolation;

const SOURCE_LIMIT: usize = 1024 * 1024;
const RESULT_LIMIT: usize = 64 * 1024;
const SHIM: &str = include_str!("shim.js");

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Command {
    pub command: String,
    pub title: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    pub kind: String,
    pub text: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Outcome {
    pub value: Value,
    pub messages: Vec<Message>,
}

pub struct Extension {
    manifest: Value,
    source: String,
    pub commands: Vec<Command>,
}

impl Extension {
    pub fn from_source(manifest: &str, source: &str) -> Result<Self, String> {
        if manifest.len() > SOURCE_LIMIT || source.len() > SOURCE_LIMIT {
            return Err("Extension exceeds 1 MiB input limit".into());
        }
        let manifest: Value =
            serde_json::from_str(manifest).map_err(|e| e.to_string())?;
        let commands: Vec<Command> = serde_json::from_value(
            manifest
                .pointer("/contributes/commands")
                .cloned()
                .ok_or("No declared commands")?,
        )
        .map_err(|e| e.to_string())?;
        if commands.is_empty() || commands.len() > 64 {
            return Err("Expected 1 to 64 declared commands".into());
        }
        let mut ids = std::collections::HashSet::new();
        for command in &commands {
            if command.command.is_empty()
                || command.command.len() > 128
                || command.title.len() > 256
                || !command
                    .command
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
                || !ids.insert(command.command.clone())
            {
                return Err("Invalid or duplicate command declaration".into());
            }
        }
        Ok(Self {
            manifest,
            source: source.into(),
            commands,
        })
    }

    /// Explicit developer load only. No marketplace discovery or auto-activation.
    pub fn load(root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|e| format!("canonicalize {}: {e}", root.display()))?;
        let read = |path: &Path| -> Result<String, String> {
            let path = path
                .canonicalize()
                .map_err(|e| format!("canonicalize {}: {e}", path.display()))?;
            if !path.starts_with(&root) {
                return Err("Extension entry escapes package".into());
            }
            let file = fs::File::open(&path)
                .map_err(|e| format!("open {}: {e}", path.display()))?;
            if !file.metadata().map_err(|e| e.to_string())?.is_file() {
                return Err("Not a regular file".into());
            }
            use std::io::Read;
            let mut bytes = Vec::new();
            file.take((SOURCE_LIMIT + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > SOURCE_LIMIT {
                return Err("Extension input too large".into());
            }
            String::from_utf8(bytes).map_err(|e| e.to_string())
        };
        let manifest = read(&root.join("package.json"))?;
        let value: Value =
            serde_json::from_str(&manifest).map_err(|e| e.to_string())?;
        let main = value
            .get("main")
            .and_then(Value::as_str)
            .ok_or("No CommonJS main entry")?;
        let entry = Path::new(main);
        if entry.is_absolute()
            || entry
                .components()
                .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
            || entry.extension().and_then(|s| s.to_str()) != Some("js")
        {
            return Err("Expected a confined .js main entry".into());
        }
        Self::from_source(&manifest, &read(&root.join(entry))?)
    }

    /// Each call has a fresh runtime. Only extension-owned commands are callable.
    fn execute_in_worker(
        &self,
        command: &str,
        args: Value,
    ) -> Result<Outcome, String> {
        if !self.commands.iter().any(|c| c.command == command) {
            return Err("Undeclared command".into());
        }
        if !args.is_array() {
            return Err("Command arguments must be a JSON array".into());
        }
        let args = serde_json::to_string(&args).map_err(|e| e.to_string())?;
        if args.len() > RESULT_LIMIT {
            return Err("Command arguments too large".into());
        }
        let runtime = Runtime::new().map_err(|e| e.to_string())?;
        runtime.set_memory_limit(16 * 1024 * 1024);
        runtime.set_max_stack_size(512 * 1024);
        let context = Context::full(&runtime).map_err(|e| e.to_string())?;
        let eval = |source: &str| -> Result<(), String> {
            context.with(|ctx| ctx.eval::<(), _>(source).map_err(|e| format!("JS error (unsupported API, exception or resource limit): {e}")))
        };
        eval(&format!("globalThis.__manifest = {};", self.manifest))?;
        eval(SHIM)?;
        eval(&format!(
            "(function(require,module,exports){{\n{}\n}})(require,module,exports);",
            self.source
        ))?;
        eval(&format!(
            "__run({}, {}).catch(() => {{globalThis.__failed = true;}});",
            serde_json::to_string(command).unwrap(),
            args
        ))?;
        while runtime.is_job_pending() {
            runtime
                .execute_pending_job()
                .map_err(|_| "JS promise job failed".to_string())?;
        }
        let text = context
            .with(|ctx| ctx.globals().get::<_, String>("__result"))
            .map_err(|_| "Command failed or did not settle".to_string())?;
        if text.len() > RESULT_LIMIT {
            return Err("Result limit exceeded".into());
        }
        let result: Outcome =
            serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if result.messages.len() > 64
            || result.messages.iter().any(|m| {
                m.text.len() > 8192
                    || !matches!(m.kind.as_str(), "info" | "warning" | "error")
            })
        {
            return Err("Invalid message output".into());
        }
        Ok(result)
    }
}

#[derive(Serialize, Deserialize)]
struct Request {
    manifest: Value,
    source: String,
    command: String,
    args: Value,
}

/// Entry for a dedicated worker mode. Never called in the UI process.
pub fn worker_main() -> i32 {
    if isolation::restrict().is_err() {
        return 2;
    }
    use std::io::{Read, Write};
    let mut bytes = Vec::new();
    if std::io::stdin()
        .take((3 * SOURCE_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > 3 * SOURCE_LIMIT
    {
        return 2;
    }
    let result = (|| -> Result<Outcome, String> {
        let request: Request =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        Extension::from_source(&request.manifest.to_string(), &request.source)?
            .execute_in_worker(&request.command, request.args)
    })();
    match serde_json::to_vec(&result) {
        Ok(bytes) if bytes.len() <= RESULT_LIMIT => {
            if std::io::stdout().write_all(&bytes).is_ok() {
                0
            } else {
                2
            }
        }
        _ => 2,
    }
}

impl Extension {
    /// Worker must be the editor's own executable, never a package-provided path.
    pub fn execute(
        &self,
        worker: &Path,
        command: &str,
        args: Value,
    ) -> Result<Outcome, String> {
        use std::{
            io::{Read, Write},
            process::{Command as ProcessCommand, Stdio},
        };
        let request = serde_json::to_vec(&Request {
            manifest: self.manifest.clone(),
            source: self.source.clone(),
            command: command.into(),
            args,
        })
        .map_err(|e| e.to_string())?;
        if request.len() > 3 * SOURCE_LIMIT {
            return Err("Worker request too large".into());
        }
        let mut child = ProcessCommand::new(worker)
            .arg("--vscode-experiment-worker")
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn worker {}: {e}", worker.display()))?;
        let mut stdin = child.stdin.take().ok_or("Missing worker stdin")?;
        let stdout = child.stdout.take().ok_or("Missing worker stdout")?;
        let writer = std::thread::spawn(move || stdin.write_all(&request));
        let reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout
                .take((RESULT_LIMIT + 1) as u64)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err("Worker time limit exceeded".to_string());
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(error.to_string());
                }
            }
        };
        let _ = writer.join();
        let bytes = reader
            .join()
            .map_err(|_| "Worker reader failed")?
            .map_err(|e| e.to_string())?;
        if !status?.success() || bytes.len() > RESULT_LIMIT {
            return Err("Worker failed or exceeded output limit".into());
        }
        let outcome = serde_json::from_slice::<Result<Outcome, String>>(&bytes)
            .map_err(|e| e.to_string())??;
        if outcome.messages.len() > 64
            || outcome.messages.iter().any(|m| {
                m.text.len() > 8192
                    || !matches!(m.kind.as_str(), "info" | "warning" | "error")
            })
        {
            return Err("Invalid worker message output".into());
        }
        Ok(outcome)
    }
}

pub fn demo(worker: &Path) -> Result<Outcome, String> {
    Extension::from_source(
        include_str!("../examples/hello/package.json"),
        include_str!("../examples/hello/extension.js"),
    )?
    .execute(worker, "sandbox.hello", serde_json::json!([]))
}
