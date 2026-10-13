//! Independent stdio LSP client. No Volt, WASI, PSP or debugger host.
use anyhow::{Context, Result, ensure};
use lapce_rpc::{
    RpcError,
    core::{CoreNotification, CoreRpcHandler},
    plugin::PluginId,
};
use lsp_types::{Position, Url};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

pub mod registry;
const MAX_MESSAGE: usize = 16 * 1024 * 1024;
type Pending =
    HashMap<u64, crossbeam_channel::Sender<std::result::Result<Value, RpcError>>>;
fn error(message: impl ToString) -> RpcError {
    RpcError {
        code: -32001,
        message: message.to_string(),
    }
}
fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>> {
    let mut length = None;
    let mut header_size = 0;
    loop {
        let mut line = String::new();
        let count = reader.take(8193).read_line(&mut line)?;
        if count == 0 {
            return Ok(None);
        }
        header_size += line.len();
        ensure!(header_size <= 8192, "Oversized LSP header");
        if line == "\r\n" || line == "\n" {
            break;
        }
        let (key, value) = line.split_once(':').context("Malformed LSP header")?;
        if key.eq_ignore_ascii_case("content-length") {
            ensure!(length.is_none(), "Duplicate Content-Length");
            length = Some(value.trim().parse::<usize>()?);
        }
    }
    let length = length.context("Missing Content-Length")?;
    ensure!(length <= MAX_MESSAGE, "Oversized LSP message");
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}
fn write_message(writer: &mut impl Write, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    ensure!(bytes.len() <= MAX_MESSAGE, "Oversized LSP message");
    write!(writer, "Content-Length: {}\r\n\r\n", bytes.len())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

pub struct Client {
    pub id: PluginId,
    stdin: Arc<Mutex<ChildStdin>>,
    child: Mutex<Child>,
    pending: Arc<Mutex<Pending>>,
    next: AtomicU64,
    pub capabilities: Value,
    documents: Mutex<HashMap<PathBuf, String>>,
}
impl Client {
    pub fn start(
        mut command: Command,
        root: Option<&Path>,
        options: Value,
        core: CoreRpcHandler,
        name: &str,
    ) -> Result<Arc<Self>> {
        if let Some(root) = root {
            command.current_dir(root);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Cannot launch language server")?;
        let stdin =
            Arc::new(Mutex::new(child.stdin.take().context("No server stdin")?));
        let stdout = child.stdout.take().context("No server stdout")?;
        let stderr = child.stderr.take().context("No server stderr")?;
        let pending: Arc<Mutex<Pending>> = Arc::new(Mutex::new(HashMap::new()));
        let responses = pending.clone();
        let writer = stdin.clone();
        let name = name.to_owned();
        let reader_core = core.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let result = (|| -> Result<()> {
                while let Some(message) = read_message(&mut reader)? {
                    if let Some(method) =
                        message.get("method").and_then(Value::as_str)
                    {
                        let params =
                            message.get("params").cloned().unwrap_or(Value::Null);
                        if let Some(id) = message.get("id") {
                            let result = match method {
                                "workspace/configuration" => Some(Value::Array(
                                    params["items"]
                                        .as_array()
                                        .map(|items| {
                                            items
                                                .iter()
                                                .map(|_| Value::Null)
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                )),
                                "workspace/workspaceFolders" => Some(Value::Null),
                                "window/workDoneProgress/create" => {
                                    Some(Value::Null)
                                }
                                _ => None,
                            };
                            let reply = match result {
                                Some(result) => {
                                    json!({"jsonrpc":"2.0", "id":id, "result":result})
                                }
                                None => {
                                    json!({"jsonrpc":"2.0", "id":id, "error":{"code":-32601,"message":"Client method not supported"}})
                                }
                            };
                            write_message(&mut *writer.lock(), &reply)?;
                        } else if method == "textDocument/publishDiagnostics" {
                            if let Ok(diagnostics) = serde_json::from_value(params) {
                                reader_core.notification(
                                    CoreNotification::PublishDiagnostics {
                                        diagnostics,
                                    },
                                );
                            }
                        } else if method == "window/showMessage" {
                            if let Ok(message) = serde_json::from_value(params) {
                                reader_core.notification(
                                    CoreNotification::ShowMessage {
                                        title: name.clone(),
                                        message,
                                    },
                                );
                            }
                        }
                    } else if let Some(id) =
                        message.get("id").and_then(Value::as_u64)
                    {
                        if let Some(sender) = responses.lock().remove(&id) {
                            let result = if let Some(err) = message.get("error") {
                                Err(RpcError {
                                    code: err["code"].as_i64().unwrap_or(-32001),
                                    message: err["message"]
                                        .as_str()
                                        .unwrap_or("Language server error")
                                        .into(),
                                })
                            } else {
                                Ok(message
                                    .get("result")
                                    .cloned()
                                    .unwrap_or(Value::Null))
                            };
                            let _ = sender.send(result);
                        }
                    }
                }
                Ok(())
            })();
            let why = result
                .err()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Language server exited".into());
            for (_, sender) in responses.lock().drain() {
                let _ = sender.send(Err(error(&why)));
            }
            tracing::warn!(%why, "LSP transport closed");
        });
        std::thread::spawn(move || {
            // Drain stderr without retaining unbounded log text.
            let mut stderr = stderr;
            let mut chunk = [0; 4096];
            while stderr.read(&mut chunk).is_ok_and(|count| count > 0) {}
        });
        let mut client = Self {
            id: PluginId::next(),
            stdin,
            child: Mutex::new(child),
            pending,
            next: AtomicU64::new(1),
            capabilities: Value::Null,
            documents: Mutex::new(HashMap::new()),
        };
        let root_uri = root.and_then(|path| Url::from_directory_path(path).ok());
        let result = client.request("initialize", json!({
            "processId": std::process::id(), "rootUri": root_uri,
            "clientInfo": {"name":"editor.izdrail.com", "version":env!("CARGO_PKG_VERSION")},
            "capabilities": {"general":{"positionEncodings":["utf-16"]}, "textDocument": {
                "synchronization":{"didSave":true},
                "completion":{"completionItem":{"snippetSupport":true,"resolveSupport":{"properties":["documentation","detail","additionalTextEdits"]}}},
                "hover":{"contentFormat":["markdown","plaintext"]},
                "publishDiagnostics":{"versionSupport":true}
            }}, "initializationOptions":options,
            "workspaceFolders": root_uri.as_ref().map(|uri| vec![json!({"uri":uri,"name":root.unwrap().file_name().unwrap_or_default().to_string_lossy()})])
        })).map_err(|e| anyhow::anyhow!(e.message))?;
        client.capabilities = result["capabilities"].clone();
        ensure!(
            client
                .capabilities
                .get("positionEncoding")
                .and_then(Value::as_str)
                .unwrap_or("utf-16")
                == "utf-16",
            "Unsupported server position encoding"
        );
        client.notify("initialized", json!({}))?;
        Ok(Arc::new(client))
    }
    pub fn request(
        &self,
        method: &str,
        params: Value,
    ) -> std::result::Result<Value, RpcError> {
        self.request_timeout(method, params, Duration::from_secs(20))
    }
    fn request_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> std::result::Result<Value, RpcError> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.pending.lock().insert(id, tx);
        if let Err(err) = write_message(
            &mut *self.stdin.lock(),
            &json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
        ) {
            self.pending.lock().remove(&id);
            return Err(error(err));
        }
        match rx.recv_timeout(timeout) {
            Ok(result) => result,
            Err(_) => {
                self.pending.lock().remove(&id);
                let _ = self.notify("$/cancelRequest", json!({"id":id}));
                Err(error("Language server request timed out"))
            }
        }
    }
    pub fn notify(&self, method: &str, params: Value) -> Result<()> {
        write_message(
            &mut *self.stdin.lock(),
            &json!({"jsonrpc":"2.0","method":method,"params":params}),
        )
    }
    pub fn alive(&self) -> bool {
        self.child
            .lock()
            .try_wait()
            .is_ok_and(|status| status.is_none())
    }
    pub fn sync(
        &self,
        path: &Path,
        language: &str,
        text: &str,
        version: u64,
    ) -> Result<()> {
        ensure!(
            text.len() < MAX_MESSAGE / 2,
            "Document exceeds LSP sync size limit"
        );
        let uri = Url::from_file_path(path)
            .map_err(|_| anyhow::anyhow!("Invalid file URI"))?;
        let version = i32::try_from(version).context("Document version overflow")?;
        let mut documents = self.documents.lock();
        match documents.get(path) {
            None => self.notify("textDocument/didOpen", json!({"textDocument":{"uri":uri,"languageId":language,"version":version,"text":text}}))?,
            Some(old) if old != text => {
                let sync = &self.capabilities["textDocumentSync"];
                let kind = sync.as_u64().or_else(|| sync["change"].as_u64()).unwrap_or(0);
                if kind == 0 { return Ok(()); }
                let change = if kind == 2 {
                    let end = utf16_end(old);
                    json!({"range":{"start":{"line":0,"character":0},"end":end},"text":text})
                } else { json!({"text":text}) };
                self.notify("textDocument/didChange", json!({"textDocument":{"uri":uri,"version":version},"contentChanges":[change]}))?;
            }
            _ => {}
        }
        documents.insert(path.to_path_buf(), text.to_owned());
        Ok(())
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        let sync = &self.capabilities["textDocumentSync"];
        if sync["save"].is_null() || sync["save"] == false {
            return Ok(());
        }
        let mut params = json!({"textDocument":{"uri":Url::from_file_path(path).map_err(|_| anyhow::anyhow!("Invalid URI"))?}});
        if sync["save"]["includeText"] == true {
            params["text"] = json!(self.documents.lock().get(path));
        }
        self.notify("textDocument/didSave", params)
    }
    pub fn close(&self, path: &Path) {
        if self.documents.lock().remove(path).is_some() {
            if let Ok(uri) = Url::from_file_path(path) {
                let _ = self.notify(
                    "textDocument/didClose",
                    json!({"textDocument":{"uri":uri}}),
                );
            }
        }
    }
    pub fn shutdown(&self) {
        let _ = self.request_timeout(
            "shutdown",
            Value::Null,
            Duration::from_millis(500),
        );
        let _ = self.notify("exit", Value::Null);
        let deadline = Instant::now() + Duration::from_millis(500);
        let mut child = self.child.lock();
        while Instant::now() < deadline
            && child.try_wait().is_ok_and(|status| status.is_none())
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let child = self.child.get_mut();
        let _ = child.kill();
        let _ = child.wait();
    }
}
fn utf16_end(text: &str) -> Position {
    let mut line = 0;
    let mut character = 0;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            line += 1;
            character = 0;
        } else if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf16() as u32;
        }
    }
    Position { line, character }
}

pub enum Job {
    Sync(crate::buffer::Buffer),
    Save(PathBuf),
    Close(PathBuf),
    Request {
        buffer: crate::buffer::Buffer,
        method: String,
        params: Value,
        callback:
            Box<dyn FnOnce(std::result::Result<(PluginId, Value), RpcError>) + Send>,
    },
    Resolve {
        id: PluginId,
        item: Value,
        callback: Box<dyn FnOnce(std::result::Result<Value, RpcError>) + Send>,
    },
    Shutdown,
}
pub struct Manager {
    sender: crossbeam_channel::Sender<Job>,
}
impl Manager {
    pub fn new(root: Option<PathBuf>, core: CoreRpcHandler) -> Self {
        Self::with_registry(root, core, registry::Registry::load())
    }
    pub(crate) fn with_registry(
        root: Option<PathBuf>,
        core: CoreRpcHandler,
        registry: Result<registry::Registry>,
    ) -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            let mut clients: HashMap<String, Arc<Client>> = HashMap::new();
            let mut failed: HashMap<String, Instant> = HashMap::new();
            let mut missing: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            let mut documents: HashMap<PathBuf, crate::buffer::Buffer> =
                HashMap::new();
            while let Ok(job) = receiver.recv() {
                let (buffer, request) = match job {
                    Job::Resolve { id, item, callback } => {
                        let client = clients.values().find(|client| client.id == id);
                        callback(
                            client
                                .map(|client| {
                                    client.request("completionItem/resolve", item)
                                })
                                .unwrap_or_else(|| {
                                    Err(error("Language server no longer available"))
                                }),
                        );
                        continue;
                    }
                    Job::Shutdown => {
                        for (_, client) in clients.drain() {
                            client.shutdown();
                        }
                        break;
                    }
                    Job::Close(path) => {
                        documents.remove(&path);
                        for client in clients.values() {
                            client.close(&path);
                        }
                        continue;
                    }
                    Job::Save(path) => {
                        for client in clients.values() {
                            let _ = client.save(&path);
                        }
                        continue;
                    }
                    Job::Sync(buffer) => (buffer, None),
                    Job::Request {
                        buffer,
                        method,
                        params,
                        callback,
                    } => (buffer, Some((method, params, callback))),
                };
                documents.insert(buffer.path.clone(), buffer.clone());
                let result = (|| -> Result<Arc<Client>> {
                    let registry = registry
                        .as_ref()
                        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
                    let (name, server) = registry
                        .server(buffer.language_id)
                        .context("No enabled language server for this file")?;
                    if clients.get(&name).is_some_and(|client| !client.alive()) {
                        clients.remove(&name);
                    }
                    if !clients.contains_key(&name) {
                        ensure!(
                            !failed.get(&name).is_some_and(
                                |at| at.elapsed() < Duration::from_secs(30)
                            ),
                            "Language server recently failed; retry in 30 seconds"
                        );
                        if registry.is_missing(&server) {
                            // Not installed and not auto-installable: stay quiet
                            // instead of popping an error on every startup.
                            if missing.insert(name.clone()) {
                                tracing::info!(
                                    server = %name,
                                    command = %server.command,
                                    "language server not found on PATH; disabled for this session"
                                );
                            }
                            anyhow::bail!(
                                "{name} language server is not installed ({} not found on PATH)",
                                server.command
                            );
                        }
                        core.notification(CoreNotification::ShowMessage {
                            title: "Language server".into(),
                            message: lsp_types::ShowMessageParams {
                                typ: lsp_types::MessageType::INFO,
                                message: format!("Preparing {name} language server"),
                            },
                        });
                        let launch =
                            registry.command(&name, &server).and_then(|command| {
                                Client::start(
                                    command,
                                    root.as_deref(),
                                    server.initialization_options,
                                    core.clone(),
                                    &name,
                                )
                            });
                        let client = match launch {
                            Ok(client) => client,
                            Err(error) => {
                                failed.insert(name.clone(), Instant::now());
                                core.notification(CoreNotification::ShowMessage {
                                    title: "Language server".into(),
                                    message: lsp_types::ShowMessageParams {
                                        typ: lsp_types::MessageType::ERROR,
                                        message: format!("{name}: {error}"),
                                    },
                                });
                                return Err(error);
                            }
                        };
                        // Reopen all retained documents after a server restart.
                        for open in documents.values().filter(|open| {
                            server
                                .languages
                                .iter()
                                .any(|language| language == open.language_id)
                        }) {
                            client.sync(
                                &open.path,
                                open.language_id,
                                &open.rope.to_string(),
                                open.rev,
                            )?;
                        }
                        clients.insert(name.clone(), client);
                    }
                    let client = clients[&name].clone();
                    client.sync(
                        &buffer.path,
                        buffer.language_id,
                        &buffer.rope.to_string(),
                        buffer.rev,
                    )?;
                    Ok(client)
                })();
                if let Some((method, params, callback)) = request {
                    callback(match result {
                        Ok(client) => client
                            .request(&method, params)
                            .map(|value| (client.id, value)),
                        Err(err) => Err(error(err)),
                    });
                }
            }
            for (_, client) in clients {
                client.shutdown();
            }
        });
        Self { sender }
    }
    pub fn send(&self, job: Job) {
        let _ = self.sender.send(job);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_unicode_and_limits() {
        let value = json!({"text":"λ🙂\r\n"});
        let mut bytes = Vec::new();
        write_message(&mut bytes, &value).unwrap();
        assert_eq!(
            read_message(&mut std::io::Cursor::new(bytes)).unwrap(),
            Some(value)
        );
        for header in [
            "Content-Length: 16777217\r\n\r\n",
            "X: x\r\n\r\n",
            "Content-Length: 1\r\nContent-Length: 1\r\n\r\n",
        ] {
            assert!(read_message(&mut std::io::Cursor::new(header)).is_err());
        }
        assert_eq!(
            utf16_end("λ🙂"),
            Position {
                line: 0,
                character: 3
            }
        );
        assert_eq!(
            utf16_end("a\r\nb\rc\n🙂"),
            Position {
                line: 3,
                character: 2
            }
        );
    }
    #[test]
    fn registry_serializes_and_selects_language_without_wasm() {
        let registry = registry::Registry::default();
        let text = toml::to_string_pretty(&registry).unwrap();
        let loaded: registry::Registry = toml::from_str(&text).unwrap();
        assert_eq!(loaded.server("python").unwrap().0, "python");
        assert!(loaded.server("unknown").is_none());
    }
    #[test]
    fn mock_stdio_roundtrip_sync_completion_hover_definition() {
        let root = tempfile::tempdir().unwrap();
        let mut command = Command::new("python3");
        command.arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/lsp/mock_server.py"
        ));
        let client = Client::start(
            command,
            Some(root.path()),
            json!({}),
            CoreRpcHandler::new(),
            "mock",
        )
        .unwrap();
        let path = root.path().join("file.py");
        let uri = Url::from_file_path(&path).unwrap();
        client.sync(&path, "python", "λ🙂", 1).unwrap();
        let params =
            json!({"textDocument":{"uri":uri},"position":{"line":0,"character":0}});
        let completion = client
            .request("textDocument/completion", params.clone())
            .unwrap();
        assert_eq!(completion[0]["label"], "mock_completion");
        let hover = client
            .request("textDocument/hover", params.clone())
            .unwrap();
        assert_eq!(hover["contents"]["value"], "λ🙂");
        client.sync(&path, "python", "changed", 2).unwrap();
        assert_eq!(
            client
                .request("textDocument/hover", params.clone())
                .unwrap()["contents"]["value"],
            "changed"
        );
        assert_eq!(
            client.request("textDocument/definition", params).unwrap()["uri"],
            uri.as_str()
        );
        assert_eq!(
            client
                .request("completionItem/resolve", completion[0].clone())
                .unwrap()["detail"],
            "resolved"
        );
        client.save(&path).unwrap();
        client.close(&path);
        client.shutdown();
        assert!(!client.alive());
    }
    #[test]
    #[ignore = "explicit real language server integration smoke"]
    fn real_pyright_features() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("smoke.py");
        let text = "def greet(name: str) -> str:\n    return name\nvalue = greet('world')\nwrong: int = 'bad'\nvalue.\n";
        std::fs::write(&path, text).unwrap();
        let mut command =
            Command::new(std::env::var("LAPCE_TEST_LSP_COMMAND").unwrap());
        command.arg("--stdio");
        let core = CoreRpcHandler::new();
        let client = Client::start(
            command,
            Some(root.path()),
            json!({}),
            core.clone(),
            "pyright",
        )
        .unwrap();
        client.sync(&path, "python", text, 1).unwrap();
        std::thread::sleep(Duration::from_secs(2));
        let uri = Url::from_file_path(&path).unwrap();
        let definition = client.request("textDocument/definition", json!({"textDocument":{"uri":uri},"position":{"line":2,"character":10}})).unwrap();
        assert!(!definition.is_null(), "{definition}");
        let hover = client.request("textDocument/hover", json!({"textDocument":{"uri":uri},"position":{"line":2,"character":10}})).unwrap();
        assert!(!hover.is_null(), "{hover}");
        let completion = client.request("textDocument/completion", json!({"textDocument":{"uri":uri},"position":{"line":4,"character":6}})).unwrap();
        assert!(
            completion["items"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
                || completion.as_array().is_some_and(|items| !items.is_empty()),
            "{completion}"
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut found = false;
        while Instant::now() < deadline {
            if let Ok(lapce_rpc::core::CoreRpc::Notification(notification)) =
                core.rx().recv_timeout(Duration::from_millis(100))
            {
                if let CoreNotification::PublishDiagnostics { diagnostics } =
                    *notification
                {
                    if diagnostics.uri == uri && !diagnostics.diagnostics.is_empty()
                    {
                        found = true;
                        break;
                    }
                }
            }
        }
        assert!(found, "real server diagnostics were not received");
        client.shutdown();
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;
    use lapce_rpc::proxy::{
        ProxyHandler, ProxyNotification, ProxyRequest, ProxyResponse,
        ProxyRpcHandler,
    };
    #[test]
    fn dispatcher_roundtrip_and_diagnostics() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("file.py");
        std::fs::write(&path, "original").unwrap();
        let core = CoreRpcHandler::new();
        let proxy = ProxyRpcHandler::new();
        let mut dispatcher =
            crate::dispatch::Dispatcher::new(core.clone(), proxy.clone());
        let registry = registry::Registry {
            enabled: true,
            auto_install: false,
            servers: HashMap::from([(
                "mock".into(),
                registry::Server {
                    languages: vec!["python".into()],
                    command: "python3".into(),
                    args: vec![
                        concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/src/lsp/mock_server.py"
                        )
                        .into(),
                    ],
                    ..registry::Server::default()
                },
            )]),
        };
        dispatcher.test_lsp(root.path().to_path_buf(), registry);
        dispatcher.handle_notification(ProxyNotification::Completion {
            request_id: 42,
            path: path.clone(),
            input: "orig".into(),
            position: Position::new(0, 0),
        });
        let mut completion = false;
        let mut diagnostics = false;
        let deadline = Instant::now() + Duration::from_secs(5);
        while !(completion && diagnostics) && Instant::now() < deadline {
            if let Ok(lapce_rpc::core::CoreRpc::Notification(n)) =
                core.rx().recv_timeout(Duration::from_millis(100))
            {
                match *n {
                    CoreNotification::CompletionResponse {
                        request_id,
                        resp,
                        ..
                    } => {
                        assert_eq!(request_id, 42);
                        assert!(matches!(
                            resp,
                            lsp_types::CompletionResponse::Array(_)
                        ));
                        completion = true;
                    }
                    CoreNotification::PublishDiagnostics { diagnostics: d } => {
                        assert_eq!(d.diagnostics[0].message, "mock diagnostic");
                        diagnostics = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(completion && diagnostics);
        for request in [
            ProxyRequest::GetHover {
                request_id: 1,
                path: path.clone(),
                position: Position::new(0, 0),
            },
            ProxyRequest::GetDefinition {
                request_id: 2,
                path: path.clone(),
                position: Position::new(0, 0),
            },
        ] {
            let (tx, rx) = crossbeam_channel::bounded(1);
            proxy.request_async(request, move |result| {
                tx.send(result).unwrap();
            });
            if let lapce_rpc::proxy::ProxyRpc::Request(id, request) =
                proxy.rx().recv().unwrap()
            {
                dispatcher.handle_request(id, request);
            }
            let response = rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
            assert!(matches!(
                response,
                ProxyResponse::HoverResponse { .. }
                    | ProxyResponse::GetDefinitionResponse { .. }
            ));
        }
        dispatcher.handle_notification(ProxyNotification::Shutdown {});
    }
}

#[cfg(test)]
mod failure_tests {
    use super::*;
    #[test]
    fn failed_server_start_returns_error_and_reaps_child() {
        let mut command = Command::new("python3");
        command.args(["-c", "import sys;sys.exit(1)"]);
        let result =
            Client::start(command, None, json!({}), CoreRpcHandler::new(), "exit");
        assert!(result.is_err());
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        let _ = self.sender.send(Job::Shutdown);
    }
}
