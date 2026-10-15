//! Adapter-agnostic Debug Adapter Protocol client.
//!
//! Layers, bottom to top:
//! * framing: [`read_message`] / [`write_message`] (`Content-Length` headers)
//! * matching: [`Pending`] pairs responses to requests by `request_seq`
//! * dispatch: [`classify`] + [`pump`] split inbound traffic into responses
//!   (resolved straight away) and events/reverse requests (queued)
//! * session: [`Session`] owns one adapter process and runs the
//!   initialize / launch|attach / breakpoints / configurationDone handshake
//! * manager: [`Manager`] maps `DapId` to sessions and forwards state to the
//!   app through `CoreRpcHandler`
//!
//! Nothing here knows about a particular language. Language crates register an
//! [`AdapterResolver`] that turns a `RunDebugConfig` into an [`AdapterSpec`].
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use lapce_rpc::{
    core::{CoreRpcHandler, LogLevel},
    dap_types::{
        self, DapEvent, DapId, DapRequest, DapResponse, DebuggerCapabilities,
        RunDebugConfig, Source, SourceBreakpoint, Stopped, ThreadId,
    },
};
use parking_lot::Mutex;
use serde_json::{Value, json};

const MAX_MESSAGE: usize = 16 * 1024 * 1024;
const MAX_HEADER: usize = 8192;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const INIT_TIMEOUT: Duration = Duration::from_secs(30);
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_STACK_LEVELS: usize = 50;

// ---------------------------------------------------------------- framing

/// Read one framed message. `Ok(None)` is a clean EOF between messages.
pub fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>> {
    let mut length = None;
    let mut header_size = 0;
    loop {
        let mut line = String::new();
        let count = reader.take(MAX_HEADER as u64 + 1).read_line(&mut line)?;
        if count == 0 {
            ensure!(header_size == 0, "EOF inside DAP header");
            return Ok(None);
        }
        header_size += line.len();
        ensure!(header_size <= MAX_HEADER, "Oversized DAP header");
        if line == "\r\n" || line == "\n" {
            break;
        }
        let (key, value) = line.split_once(':').context("Malformed DAP header")?;
        if key.eq_ignore_ascii_case("content-length") {
            ensure!(length.is_none(), "Duplicate Content-Length");
            length = Some(value.trim().parse::<usize>()?);
        }
    }
    let length = length.context("Missing Content-Length")?;
    ensure!(length <= MAX_MESSAGE, "Oversized DAP message");
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}

pub fn write_message(writer: &mut impl Write, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    write!(writer, "Content-Length: {}\r\n\r\n", bytes.len())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}

// --------------------------------------------------------------- dispatch

/// An event the typed `DapEvent` enum does not know about (or failed to
/// parse). Kept so new adapter events never kill the session.
#[derive(Debug, Clone)]
pub enum Event {
    Known(DapEvent),
    Unknown { name: String, body: Value },
}

#[derive(Debug, Clone)]
pub enum Incoming {
    Response(DapResponse),
    Event(Event),
    /// Reverse request (adapter -> client), e.g. `runInTerminal`.
    Request(DapRequest),
}

pub fn classify(value: Value) -> Result<Incoming> {
    let ty = value
        .get("type")
        .and_then(Value::as_str)
        .context("DAP message without type")?
        .to_string();
    match ty.as_str() {
        "response" => Ok(Incoming::Response(serde_json::from_value(value)?)),
        "request" => Ok(Incoming::Request(serde_json::from_value(value)?)),
        "event" => {
            let name = value
                .get("event")
                .and_then(Value::as_str)
                .context("DAP event without name")?
                .to_string();
            let body = value.get("body").cloned().unwrap_or(Value::Null);
            // Adjacently tagged enum: rebuild exactly {event, body}.
            let mut tagged = json!({ "event": name });
            if !body.is_null() {
                tagged["body"] = body.clone();
            }
            Ok(Incoming::Event(match serde_json::from_value(tagged) {
                Ok(event) => Event::Known(event),
                Err(_) => Event::Unknown { name, body },
            }))
        }
        other => bail!("Unknown DAP message type {other}"),
    }
}

type PendingSender = Sender<Result<DapResponse, String>>;

/// Request/response matching table plus the outgoing sequence counter.
#[derive(Default)]
pub struct Pending {
    seq: AtomicU64,
    map: Mutex<HashMap<u64, PendingSender>>,
    closed: AtomicBool,
}

impl Pending {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate the next seq (starting at 1) and register a waiter for it.
    pub fn register(&self) -> Result<(u64, Receiver<Result<DapResponse, String>>)> {
        let mut map = self.map.lock();
        if self.closed.load(Ordering::SeqCst) {
            bail!("debug adapter connection closed");
        }
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let (tx, rx) = bounded(1);
        map.insert(seq, tx);
        Ok((seq, rx))
    }

    pub fn forget(&self, seq: u64) {
        self.map.lock().remove(&seq);
    }

    /// Deliver a response. Returns false if nobody was waiting.
    pub fn resolve(&self, response: DapResponse) -> bool {
        match self.map.lock().remove(&response.request_seq) {
            Some(tx) => tx.send(Ok(response)).is_ok(),
            None => false,
        }
    }

    /// Connection gone: wake every waiter with an error, refuse new ones.
    pub fn fail_all(&self, reason: &str) {
        let mut map = self.map.lock();
        self.closed.store(true, Ordering::SeqCst);
        for (_, tx) in map.drain() {
            let _ = tx.send(Err(reason.to_string()));
        }
    }

    pub fn len(&self) -> usize {
        self.map.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// What the reader thread hands to the event thread.
#[derive(Debug)]
pub enum Inbound {
    Event(Event),
    Request(DapRequest),
    Closed(String),
}

/// Read until EOF/error, resolving responses and queueing everything else.
/// Always finishes by failing pending requests and sending `Inbound::Closed`.
pub fn pump(mut reader: impl BufRead, pending: &Pending, inbound: &Sender<Inbound>) {
    let reason = loop {
        match read_message(&mut reader) {
            Ok(Some(value)) => match classify(value) {
                Ok(Incoming::Response(response)) => {
                    if !pending.resolve(response.clone()) {
                        tracing::debug!(
                            "unmatched DAP response to seq {}",
                            response.request_seq
                        );
                    }
                }
                Ok(Incoming::Event(event)) => {
                    let _ = inbound.send(Inbound::Event(event));
                }
                Ok(Incoming::Request(request)) => {
                    let _ = inbound.send(Inbound::Request(request));
                }
                Err(err) => tracing::warn!("bad DAP message: {err:#}"),
            },
            Ok(None) => break "debug adapter closed its output".to_string(),
            Err(err) => break format!("debug adapter protocol error: {err:#}"),
        }
    };
    pending.fail_all(&reason);
    let _ = inbound.send(Inbound::Closed(reason));
}

// -------------------------------------------------------------- transport

/// Writer half + matching table. Cheap to share via `Arc`.
pub struct Transport {
    writer: Mutex<Box<dyn Write + Send>>,
    pub pending: Arc<Pending>,
}

impl Transport {
    pub fn new(writer: Box<dyn Write + Send>, pending: Arc<Pending>) -> Self {
        Self {
            writer: Mutex::new(writer),
            pending,
        }
    }

    fn write(&self, value: &Value) -> Result<()> {
        write_message(&mut *self.writer.lock(), value)
    }

    /// Send a request and return the receiver for its response.
    pub fn send_request(
        &self,
        command: &str,
        arguments: Option<Value>,
    ) -> Result<(u64, Receiver<Result<DapResponse, String>>)> {
        let (seq, rx) = self.pending.register()?;
        let mut message = json!({"seq": seq, "type": "request", "command": command});
        if let Some(arguments) = arguments {
            message["arguments"] = arguments;
        }
        if let Err(err) = self.write(&message) {
            self.pending.forget(seq);
            return Err(err);
        }
        Ok((seq, rx))
    }

    /// Send and wait. A `success:false` response becomes an error.
    pub fn request_raw(
        &self,
        command: &str,
        arguments: Option<Value>,
        timeout: Duration,
    ) -> Result<Value> {
        let (seq, rx) = self.send_request(command, arguments)?;
        wait_response(&self.pending, seq, command, &rx, timeout)
    }

    pub fn request<R: dap_types::Request>(
        &self,
        arguments: R::Arguments,
    ) -> Result<R::Result> {
        let arguments = serde_json::to_value(arguments)?;
        // `()` arguments serialize to null: send no arguments object.
        let arguments = (!arguments.is_null()).then_some(arguments);
        let body = self.request_raw(R::COMMAND, arguments, REQUEST_TIMEOUT)?;
        Ok(serde_json::from_value(body)
            .with_context(|| format!("bad {} response body", R::COMMAND))?)
    }

    /// Answer a reverse request from the adapter.
    pub fn respond(
        &self,
        request: &DapRequest,
        result: std::result::Result<Value, String>,
    ) -> Result<()> {
        let mut message = json!({
            "seq": self.pending.seq.fetch_add(1, Ordering::SeqCst) + 1,
            "type": "response",
            "request_seq": request.seq,
            "command": request.command,
        });
        match result {
            Ok(body) => {
                message["success"] = json!(true);
                if !body.is_null() {
                    message["body"] = body;
                }
            }
            Err(error) => {
                message["success"] = json!(false);
                message["message"] = json!(error);
            }
        }
        self.write(&message)
    }
}

pub fn wait_response(
    pending: &Pending,
    seq: u64,
    command: &str,
    rx: &Receiver<Result<DapResponse, String>>,
    timeout: Duration,
) -> Result<Value> {
    match rx.recv_timeout(timeout) {
        Ok(Ok(response)) => {
            if response.success {
                Ok(response.body.unwrap_or(Value::Null))
            } else {
                Err(anyhow!(
                    "{command} failed: {}",
                    response.message.unwrap_or_else(|| "unknown error".into())
                ))
            }
        }
        Ok(Err(reason)) => Err(anyhow!("{command}: {reason}")),
        Err(_) => {
            pending.forget(seq);
            Err(anyhow!("{command} timed out"))
        }
    }
}

// ---------------------------------------------------------------- adapter

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestKind {
    #[default]
    Launch,
    Attach,
}

impl RequestKind {
    fn command(self) -> &'static str {
        match self {
            Self::Launch => "launch",
            Self::Attach => "attach",
        }
    }
}

/// How to start one debug adapter and what to send it.
#[derive(Debug, Clone, Default)]
pub struct AdapterSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    /// DAP `adapterID` sent in `initialize`.
    pub adapter_id: String,
    pub request: RequestKind,
    /// Merged over the generic launch/attach arguments (adapter specifics).
    pub extra_arguments: Option<Value>,
}

impl AdapterSpec {
    pub fn from_dap_server(
        server: dap_types::DapServer,
        adapter_id: String,
    ) -> Self {
        Self {
            program: server.program,
            args: server.args,
            cwd: server.cwd,
            adapter_id,
            ..Default::default()
        }
    }
}

pub type AdapterResolver =
    dyn Fn(&RunDebugConfig) -> Option<AdapterSpec> + Send + Sync + 'static;

/// Default resolution: `config.debug_command` is the adapter command line.
pub fn default_adapter(config: &RunDebugConfig) -> Option<AdapterSpec> {
    let command = config.debug_command.as_ref()?;
    let (program, args) = command.split_first()?;
    Some(AdapterSpec {
        program: program.clone(),
        args: args.to_vec(),
        adapter_id: config.ty.clone().unwrap_or_else(|| "generic".into()),
        ..Default::default()
    })
}

/// Generic launch/attach arguments built from a run config.
pub fn launch_arguments(config: &RunDebugConfig, spec: &AdapterSpec) -> Value {
    let mut arguments = json!({
        "type": config.ty,
        "name": config.name,
        "request": spec.request.command(),
        "args": config.args.clone().unwrap_or_default(),
        "cwd": config.cwd,
        "env": config.env.clone().unwrap_or_default(),
    });
    if !config.program.is_empty() {
        arguments["program"] = json!(config.program);
    }
    if let Some(Value::Object(extra)) = &spec.extra_arguments {
        for (key, value) in extra {
            arguments[key] = value.clone();
        }
    }
    arguments
}

pub fn initialize_arguments(adapter_id: &str) -> dap_types::InitializeParams {
    dap_types::InitializeParams {
        client_id: Some("lapce".into()),
        client_name: Some("editor.izdrail.com".into()),
        adapter_id: adapter_id.to_string(),
        locale: Some("en-US".into()),
        lines_start_at_one: Some(true),
        columns_start_at_one: Some(true),
        path_format: Some("path".into()),
        supports_variable_type: Some(true),
        supports_variable_paging: Some(false),
        supports_run_in_terminal_request: Some(false),
        supports_memory_references: Some(false),
        supports_progress_reporting: Some(false),
        supports_invalidated_event: Some(false),
    }
}

fn log_level_for(category: Option<&str>) -> LogLevel {
    match category {
        Some("stderr") => LogLevel::Warn,
        Some("important") => LogLevel::Error,
        Some("telemetry") => LogLevel::Trace,
        _ => LogLevel::Info,
    }
}

// ---------------------------------------------------------------- session

pub struct Session {
    pub dap_id: DapId,
    pub config: RunDebugConfig,
    transport: Arc<Transport>,
    child: Mutex<Option<Child>>,
    capabilities: Mutex<DebuggerCapabilities>,
    initialized: (Sender<()>, Receiver<()>),
    ended: AtomicBool,
    cleaned: AtomicBool,
    core: CoreRpcHandler,
}

impl Session {
    /// Spawn the adapter and the reader/event threads. Does not handshake.
    pub fn spawn(
        dap_id: DapId,
        config: RunDebugConfig,
        spec: &AdapterSpec,
        core: CoreRpcHandler,
    ) -> Result<Arc<Self>> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .envs(&spec.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }
        let mut child = command.spawn().with_context(|| {
            format!("failed to start debug adapter {}", spec.program)
        })?;
        let stdin: ChildStdin = child.stdin.take().context("adapter stdin")?;
        let stdout = child.stdout.take().context("adapter stdout")?;
        let stderr = child.stderr.take().context("adapter stderr")?;
        let session = Self::from_io(
            dap_id,
            config,
            Box::new(stdin),
            Box::new(BufReader::new(stdout)),
            core.clone(),
        );
        *session.child.lock() = Some(child);
        if session.is_ended() {
            session.shutdown();
        }
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(|l| l.ok()) {
                tracing::debug!(target: "dap", "adapter stderr: {line}");
                core.log(LogLevel::Debug, line, Some("dap".into()));
            }
        });
        Ok(session)
    }

    /// Build a session over arbitrary pipes (the tests use in-memory ones).
    pub fn from_io(
        dap_id: DapId,
        config: RunDebugConfig,
        writer: Box<dyn Write + Send>,
        reader: Box<dyn BufRead + Send>,
        core: CoreRpcHandler,
    ) -> Arc<Self> {
        let pending = Arc::new(Pending::new());
        let transport = Arc::new(Transport::new(writer, pending.clone()));
        let (inbound_tx, inbound_rx) = unbounded();
        let session = Arc::new(Self {
            dap_id,
            config,
            transport,
            child: Mutex::new(None),
            capabilities: Mutex::new(empty_capabilities()),
            initialized: bounded(1),
            ended: AtomicBool::new(false),
            cleaned: AtomicBool::new(false),
            core,
        });
        std::thread::spawn(move || pump(reader, &pending, &inbound_tx));
        let weak = Arc::downgrade(&session);
        std::thread::spawn(move || {
            for inbound in inbound_rx {
                let Some(session) = weak.upgrade() else { break };
                let closed = matches!(inbound, Inbound::Closed(_));
                session.handle_inbound(inbound);
                if closed {
                    break;
                }
            }
        });
        session
    }

    pub fn capabilities(&self) -> DebuggerCapabilities {
        self.capabilities.lock().clone()
    }

    pub fn is_ended(&self) -> bool {
        self.ended.load(Ordering::SeqCst)
    }

    /// initialize, launch/attach, breakpoints, configurationDone.
    pub fn handshake(
        &self,
        spec: &AdapterSpec,
        breakpoints: &HashMap<PathBuf, Vec<SourceBreakpoint>>,
    ) -> Result<()> {
        let capabilities = self.transport.request::<dap_types::Initialize>(
            initialize_arguments(&spec.adapter_id),
        )?;
        *self.capabilities.lock() = capabilities;

        // Adapters answer launch/attach only after configurationDone, so do
        // not block on it here.
        let (launch_seq, launch_rx) = self.transport.send_request(
            spec.request.command(),
            Some(launch_arguments(&self.config, spec)),
        )?;

        let init_rx = self.initialized.1.clone();
        let mut launch_complete = false;
        let deadline = std::time::Instant::now() + INIT_TIMEOUT;
        loop {
            // Once launch has answered, wait only for initialized. A disconnected
            // launch receiver must not win select repeatedly and skip setup.
            if launch_complete {
                init_rx
                    .recv_timeout(
                        deadline
                            .saturating_duration_since(std::time::Instant::now()),
                    )
                    .context("debug adapter never sent initialized")?;
                break;
            }
            crossbeam_channel::select! {
                recv(init_rx) -> initialized => { initialized.context("initialized channel closed")?; break; }
                recv(launch_rx) -> response => {
                    // Launch answered before `initialized`: either an error, or
                    // an adapter that skips the event. Errors abort.
                    match response {
                        Ok(Ok(response)) if !response.success => bail!(
                            "{} failed: {}",
                            spec.request.command(),
                            response.message.unwrap_or_default()
                        ),
                        Ok(Err(reason)) => bail!(reason),
                        Ok(Ok(_)) => launch_complete = true,
                        Err(_) => bail!("debug adapter closed before initialized"),
                    }
                }
                default(deadline.saturating_duration_since(std::time::Instant::now())) => {
                    self.transport.pending.forget(launch_seq);
                    bail!("debug adapter never sent `initialized`");
                }
            }
        }

        for (path, lines) in breakpoints {
            self.set_breakpoints(path, lines.clone())
                .with_context(|| format!("setBreakpoints {}", path.display()))?;
        }
        if self
            .capabilities()
            .supports_configuration_done_request
            .unwrap_or(false)
        {
            self.transport.request::<dap_types::ConfigurationDone>(())?;
        }
        if !launch_complete {
            wait_response(
                &self.transport.pending,
                launch_seq,
                spec.request.command(),
                &launch_rx,
                LAUNCH_TIMEOUT,
            )?;
        }
        Ok(())
    }

    pub fn set_breakpoints(
        &self,
        path: &Path,
        breakpoints: Vec<SourceBreakpoint>,
    ) -> Result<Vec<dap_types::Breakpoint>> {
        let response = self.transport.request::<dap_types::SetBreakpoints>(
            dap_types::SetBreakpointsArguments {
                source: Source {
                    name: path.file_name().map(|n| n.to_string_lossy().into_owned()),
                    path: Some(path.to_path_buf()),
                    ..Default::default()
                },
                breakpoints: Some(breakpoints),
                source_modified: None,
            },
        )?;
        let breakpoints = response.breakpoints.unwrap_or_default();
        self.core.dap_breakpoints_resp(
            self.dap_id,
            path.to_path_buf(),
            breakpoints.clone(),
        );
        Ok(breakpoints)
    }

    pub fn threads(&self) -> Result<Vec<dap_types::Thread>> {
        Ok(self.transport.request::<dap_types::Threads>(())?.threads)
    }

    pub fn stack_trace(
        &self,
        thread_id: ThreadId,
    ) -> Result<Vec<dap_types::StackFrame>> {
        Ok(self
            .transport
            .request::<dap_types::StackTrace>(dap_types::StackTraceArguments {
                thread_id,
                levels: Some(MAX_STACK_LEVELS),
                ..Default::default()
            })?
            .stack_frames)
    }

    pub fn scopes(&self, frame_id: usize) -> Result<Vec<dap_types::Scope>> {
        Ok(self
            .transport
            .request::<dap_types::Scopes>(dap_types::ScopesArguments { frame_id })?
            .scopes)
    }

    pub fn variables(&self, reference: usize) -> Result<Vec<dap_types::Variable>> {
        Ok(self
            .transport
            .request::<dap_types::Variables>(dap_types::VariablesArguments {
                variables_reference: reference,
                ..Default::default()
            })?
            .variables)
    }

    /// Scopes of a frame with their variables (expensive scopes stay lazy).
    pub fn scopes_with_variables(
        &self,
        frame_id: usize,
    ) -> Result<Vec<(dap_types::Scope, Vec<dap_types::Variable>)>> {
        let mut out = Vec::new();
        for scope in self.scopes(frame_id)? {
            let variables = if scope.expensive || scope.variables_reference == 0 {
                Vec::new()
            } else {
                self.variables(scope.variables_reference)
                    .unwrap_or_else(|err| {
                        tracing::warn!("variables: {err:#}");
                        Vec::new()
                    })
            };
            out.push((scope, variables));
        }
        Ok(out)
    }

    pub fn continue_(&self, thread_id: ThreadId) -> Result<()> {
        self.transport.request::<dap_types::Continue>(
            dap_types::ContinueArguments { thread_id },
        )?;
        Ok(())
    }

    pub fn next(&self, thread_id: ThreadId) -> Result<()> {
        self.transport
            .request::<dap_types::Next>(dap_types::NextArguments {
                thread_id,
                granularity: None,
            })
    }

    pub fn step_in(&self, thread_id: ThreadId) -> Result<()> {
        self.transport
            .request::<dap_types::StepIn>(dap_types::StepInArguments {
                thread_id,
                target_id: None,
                granularity: None,
            })
    }

    pub fn step_out(&self, thread_id: ThreadId) -> Result<()> {
        self.transport
            .request::<dap_types::StepOut>(dap_types::StepOutArguments {
                thread_id,
                granularity: None,
            })
    }

    pub fn pause(&self, thread_id: ThreadId) -> Result<()> {
        self.transport
            .request::<dap_types::Pause>(dap_types::PauseArguments { thread_id })
    }

    /// Stop the debuggee: `terminate` when supported, else `disconnect`.
    pub fn terminate(&self) -> Result<()> {
        if self
            .capabilities()
            .supports_terminate_request
            .unwrap_or(false)
        {
            self.transport.request::<dap_types::Terminate>(())
        } else {
            self.disconnect(true)
        }
    }

    pub fn disconnect(&self, terminate_debuggee: bool) -> Result<()> {
        self.transport
            .request_raw(
                "disconnect",
                Some(json!({"terminateDebuggee": terminate_debuggee})),
                Duration::from_secs(5),
            )
            .map(|_| ())
    }

    /// Best-effort full teardown; idempotent.
    pub fn shutdown(&self) {
        if self.cleaned.swap(true, Ordering::SeqCst) {
            return;
        }
        self.ended.store(true, Ordering::SeqCst);
        // A detach must not be followed by a second disconnect that kills
        // the debuggee. Stop/terminate is requested explicitly by the caller.
        let _ = self.disconnect(false);
        if let Some(mut child) = self.child.lock().take() {
            for _ in 0..25 {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    // ---- inbound events (event thread; may issue blocking requests) ----

    fn handle_inbound(&self, inbound: Inbound) {
        match inbound {
            Inbound::Event(event) => self.handle_event(event),
            Inbound::Request(request) => {
                // We advertise no reverse-request support.
                let _ = self.transport.respond(
                    &request,
                    Err(format!("unsupported request {}", request.command)),
                );
            }
            Inbound::Closed(reason) => {
                if !self.ended.swap(true, Ordering::SeqCst) {
                    self.core.log(
                        LogLevel::Info,
                        format!("Debug session ended: {reason}"),
                        Some("dap".into()),
                    );
                    self.core.dap_continued(self.dap_id);
                }
            }
        }
    }

    pub fn handle_event(&self, event: Event) {
        let event = match event {
            Event::Known(event) => event,
            Event::Unknown { name, .. } => {
                tracing::debug!("ignoring DAP event {name}");
                return;
            }
        };
        match event {
            DapEvent::Initialized(capabilities) => {
                if let Some(capabilities) = capabilities {
                    *self.capabilities.lock() = capabilities;
                }
                let _ = self.initialized.0.try_send(());
            }
            DapEvent::Capabilities(update) => {
                *self.capabilities.lock() = update.capabilities;
            }
            DapEvent::Stopped(stopped) => {
                if let Err(err) = self.report_stopped(stopped) {
                    tracing::warn!("reporting stop: {err:#}");
                }
            }
            DapEvent::Continued(_) => self.core.dap_continued(self.dap_id),
            DapEvent::Output(output) => {
                let level = log_level_for(output.category.as_deref());
                self.core.log(level, output.output, Some("dap".into()));
            }
            DapEvent::Exited(exited) => self.core.log(
                LogLevel::Info,
                format!("Debuggee exited with code {}", exited.exit_code),
                Some("dap".into()),
            ),
            DapEvent::Terminated(_) => {
                self.core.log(
                    LogLevel::Info,
                    "Debug session terminated".into(),
                    Some("dap".into()),
                );
                self.core.dap_continued(self.dap_id);
                self.ended.store(true, Ordering::SeqCst);
            }
            DapEvent::Thread { .. }
            | DapEvent::Breakpoint { .. }
            | DapEvent::Module { .. }
            | DapEvent::LoadedSource { .. }
            | DapEvent::Process(_)
            | DapEvent::Memory(_) => {}
        }
    }

    fn report_stopped(&self, stopped: Stopped) -> Result<()> {
        let all = stopped.all_threads_stopped.unwrap_or(false)
            || stopped.thread_id.is_none();
        let mut stack_frames = HashMap::new();
        let mut top_thread = stopped.thread_id;
        if all {
            for thread in self.threads()? {
                top_thread.get_or_insert(thread.id);
                if let Ok(frames) = self.stack_trace(thread.id) {
                    stack_frames.insert(thread.id, frames);
                }
            }
        } else if let Some(id) = stopped.thread_id {
            stack_frames.insert(id, self.stack_trace(id)?);
        }
        let variables = top_thread
            .and_then(|id| stack_frames.get(&id))
            .and_then(|frames| frames.first())
            .map(|frame| self.scopes_with_variables(frame.id))
            .transpose()?
            .unwrap_or_default();
        self.core
            .dap_stopped(self.dap_id, stopped, stack_frames, variables);
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.lock().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn empty_capabilities() -> DebuggerCapabilities {
    serde_json::from_value(json!({})).expect("empty capabilities")
}

// ---------------------------------------------------------------- manager

/// Owns all debug sessions of one proxy.
pub struct Manager {
    core: CoreRpcHandler,
    sessions: Sessions,
    starts: Starts,
    resolver: Arc<AdapterResolver>,
}

impl Manager {
    pub fn new(core: CoreRpcHandler) -> Self {
        Self::with_resolver(core, Arc::new(default_adapter))
    }

    /// Install a language-aware adapter lookup (falls back to nothing).
    pub fn with_resolver(
        core: CoreRpcHandler,
        resolver: Arc<AdapterResolver>,
    ) -> Self {
        Self {
            core,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            starts: Arc::new(Mutex::new(HashMap::new())),
            resolver,
        }
    }

    pub fn session(&self, dap_id: DapId) -> Option<Arc<Session>> {
        self.sessions
            .lock()
            .get(&dap_id)
            .filter(|s| !s.is_ended())
            .cloned()
    }

    fn session_or_log(&self, dap_id: DapId) -> Option<Arc<Session>> {
        let session = self.session(dap_id);
        if session.is_none() {
            tracing::warn!("no debug session {dap_id:?}");
        }
        session
    }

    /// Start a session on its own thread; failures go to the app log.
    pub fn start(
        &self,
        config: RunDebugConfig,
        breakpoints: HashMap<PathBuf, Vec<SourceBreakpoint>>,
        workspace: Option<PathBuf>,
    ) {
        start_session(
            self.core.clone(),
            self.sessions.clone(),
            self.starts.clone(),
            self.resolver.clone(),
            config,
            breakpoints,
            workspace,
            None,
        );
    }

    /// Run `f` for a session on a worker thread so the dispatcher never blocks.
    fn with_session(
        &self,
        dap_id: DapId,
        what: &'static str,
        f: impl FnOnce(&Session) -> Result<()> + Send + 'static,
    ) {
        let Some(session) = self.session_or_log(dap_id) else {
            return;
        };
        let core = self.core.clone();
        std::thread::spawn(move || {
            if let Err(err) = f(&session) {
                core.log(
                    LogLevel::Warn,
                    format!("{what}: {err:#}"),
                    Some("dap".into()),
                );
            }
        });
    }

    pub fn continue_(&self, dap_id: DapId, thread_id: ThreadId) {
        self.with_session(dap_id, "continue", move |s| s.continue_(thread_id));
    }
    pub fn step_over(&self, dap_id: DapId, thread_id: ThreadId) {
        self.with_session(dap_id, "next", move |s| s.next(thread_id));
    }
    pub fn step_into(&self, dap_id: DapId, thread_id: ThreadId) {
        self.with_session(dap_id, "stepIn", move |s| s.step_in(thread_id));
    }
    pub fn step_out(&self, dap_id: DapId, thread_id: ThreadId) {
        self.with_session(dap_id, "stepOut", move |s| s.step_out(thread_id));
    }
    pub fn pause(&self, dap_id: DapId, thread_id: ThreadId) {
        self.with_session(dap_id, "pause", move |s| s.pause(thread_id));
    }
    pub fn set_breakpoints(
        &self,
        dap_id: DapId,
        path: PathBuf,
        breakpoints: Vec<SourceBreakpoint>,
    ) {
        self.with_session(dap_id, "setBreakpoints", move |s| {
            s.set_breakpoints(&path, breakpoints).map(|_| ())
        });
    }

    /// Stop the debuggee and drop the session.
    pub fn stop(&self, dap_id: DapId) {
        self.starts.lock().remove(&dap_id);
        let Some(session) = self.sessions.lock().remove(&dap_id) else {
            return;
        };
        let core = self.core.clone();
        std::thread::spawn(move || {
            let _ = session.terminate();
            session.shutdown();
            core.dap_continued(dap_id);
        });
    }

    pub fn disconnect(&self, dap_id: DapId) {
        self.starts.lock().remove(&dap_id);
        let Some(session) = self.sessions.lock().remove(&dap_id) else {
            return;
        };
        std::thread::spawn(move || {
            let _ = session.disconnect(false);
            session.shutdown();
        });
    }

    /// Restart: tear down the old session and start a fresh one with the
    /// same config and id (adapters rarely implement `restart` faithfully).
    pub fn restart(
        &self,
        dap_id: DapId,
        breakpoints: HashMap<PathBuf, Vec<SourceBreakpoint>>,
        workspace: Option<PathBuf>,
    ) {
        let Some(session) = self.sessions.lock().remove(&dap_id) else {
            return;
        };
        let generation = reserve_start(&self.starts, dap_id);
        let config = session.config.clone();
        let core = self.core.clone();
        let sessions = self.sessions.clone();
        let starts = self.starts.clone();
        let resolver = self.resolver.clone();
        std::thread::spawn(move || {
            let _ = session.terminate();
            session.shutdown();
            start_session(
                core,
                sessions,
                starts,
                resolver,
                config,
                breakpoints,
                workspace,
                Some(generation),
            );
        });
    }

    /// Blocking helper for `ProxyRequest::DapVariable`.
    pub fn variables(
        &self,
        dap_id: DapId,
        reference: usize,
    ) -> Result<Vec<dap_types::Variable>> {
        self.session(dap_id)
            .context("no such debug session")?
            .variables(reference)
    }

    /// Blocking helper for `ProxyRequest::DapGetScopes`.
    pub fn scopes(
        &self,
        dap_id: DapId,
        frame_id: usize,
    ) -> Result<Vec<(dap_types::Scope, Vec<dap_types::Variable>)>> {
        let session = self.session(dap_id).context("no such debug session")?;
        let mut out = Vec::new();
        for scope in session.scopes(frame_id)? {
            let variables = if scope.expensive || scope.variables_reference == 0 {
                Vec::new()
            } else {
                session.variables(scope.variables_reference)?
            };
            out.push((scope, variables));
        }
        Ok(out)
    }

    pub fn shutdown_all(&self) {
        self.starts.lock().clear();
        let all: Vec<_> = self.sessions.lock().drain().map(|(_, s)| s).collect();
        for session in all {
            session.shutdown();
        }
    }
}

type Sessions = Arc<Mutex<HashMap<DapId, Arc<Session>>>>;
type Starts = Arc<Mutex<HashMap<DapId, u64>>>;

fn reserve_start(starts: &Starts, id: DapId) -> u64 {
    static GENERATION: AtomicU64 = AtomicU64::new(0);
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst);
    starts.lock().insert(id, generation);
    generation
}

fn start_session(
    core: CoreRpcHandler,
    sessions: Sessions,
    starts: Starts,
    resolver: Arc<AdapterResolver>,
    mut config: RunDebugConfig,
    breakpoints: HashMap<PathBuf, Vec<SourceBreakpoint>>,
    workspace: Option<PathBuf>,
    generation: Option<u64>,
) {
    let dap_id = config.dap_id;
    let generation = generation.unwrap_or_else(|| reserve_start(&starts, dap_id));
    std::thread::spawn(move || {
        if starts.lock().get(&dap_id) != Some(&generation) {
            return;
        }
        let Some(mut spec) = resolver(&config) else {
            core.log(
                LogLevel::Error,
                format!(
                    "No debug adapter is configured for '{}'",
                    config.ty.as_deref().unwrap_or(&config.name)
                ),
                Some("dap".into()),
            );
            let mut pending = starts.lock();
            if pending.get(&dap_id) == Some(&generation) {
                pending.remove(&dap_id);
            }
            return;
        };
        if spec.cwd.is_none() {
            spec.cwd = config.cwd.as_ref().map(PathBuf::from).or(workspace);
        }
        if config.cwd.is_none() {
            config.cwd = spec.cwd.as_ref().map(|p| p.to_string_lossy().into_owned());
        }
        let result = (|| {
            let current = starts.lock();
            if current.get(&dap_id) != Some(&generation) {
                return Ok(());
            }
            let session = Session::spawn(dap_id, config, &spec, core.clone())?;
            let old = sessions.lock().insert(dap_id, session.clone());
            drop(current);
            if let Some(old) = old {
                old.shutdown();
            }
            if let Err(err) = session.handshake(&spec, &breakpoints) {
                session.shutdown();
                let mut active = sessions.lock();
                if active
                    .get(&dap_id)
                    .is_some_and(|s| Arc::ptr_eq(s, &session))
                {
                    active.remove(&dap_id);
                }
                return Err(err);
            }
            while !session.is_ended() {
                std::thread::sleep(Duration::from_millis(50));
            }
            session.shutdown();
            let mut active = sessions.lock();
            if active
                .get(&dap_id)
                .is_some_and(|s| Arc::ptr_eq(s, &session))
            {
                active.remove(&dap_id);
            }
            Ok(())
        })();
        {
            let mut pending = starts.lock();
            if pending.get(&dap_id) == Some(&generation) {
                pending.remove(&dap_id);
            }
        }
        if let Err(err) = result {
            core.log(
                LogLevel::Error,
                format!("Failed to start debugging: {err:#}"),
                Some("dap".into()),
            );
        }
    });
}

impl Drop for Manager {
    fn drop(&mut self) {
        self.shutdown_all();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use lapce_rpc::core::{CoreNotification, CoreRpc};
    use std::io::Cursor;

    fn frame(value: &Value) -> Vec<u8> {
        let mut out = Vec::new();
        write_message(&mut out, value).unwrap();
        out
    }

    #[test]
    fn framing_roundtrip_unicode_and_limits() {
        let value =
            json!({"seq":1,"type":"event","event":"output","body":{"output":"λ🙂"}});
        let mut cursor = Cursor::new([frame(&value), frame(&value)].concat());
        assert_eq!(read_message(&mut cursor).unwrap().unwrap(), value);
        assert_eq!(read_message(&mut cursor).unwrap().unwrap(), value);
        assert!(read_message(&mut cursor).unwrap().is_none());

        // extra headers and case-insensitive names are fine
        let body = br#"{"a":1}"#;
        let raw = format!(
            "content-type: x\r\nCONTENT-LENGTH: {}\r\n\r\n{}",
            body.len(),
            std::str::from_utf8(body).unwrap()
        );
        assert_eq!(
            read_message(&mut Cursor::new(raw.into_bytes()))
                .unwrap()
                .unwrap(),
            json!({"a":1})
        );

        let err = |raw: &str| {
            read_message(&mut Cursor::new(raw.as_bytes().to_vec())).is_err()
        };
        assert!(err("\r\n{}"), "missing length");
        assert!(
            err("Content-Length: 1\r\nContent-Length: 1\r\n\r\n{}"),
            "duplicate"
        );
        assert!(err("Content-Length: 99999999999\r\n\r\n"), "oversized");
        assert!(err("Content-Length: 10\r\n\r\n{}"), "truncated body");
        assert!(err("Content-Length: nope\r\n\r\n"), "bad number");
        assert!(err("NoColon\r\n\r\n"), "bad header");
        assert!(
            err(&format!("X: {}\r\n\r\n", "a".repeat(9000))),
            "header size"
        );
        assert!(err("Content-Length: 2"), "eof inside header");
    }

    #[test]
    fn classify_responses_events_and_requests() {
        let Incoming::Response(r) = classify(json!({
            "seq":5,"type":"response","request_seq":2,"success":true,
            "command":"threads","body":{"threads":[]}
        }))
        .unwrap() else {
            panic!()
        };
        assert_eq!((r.request_seq, r.success), (2, true));

        let Incoming::Event(Event::Known(DapEvent::Initialized(None))) =
            classify(json!({"seq":1,"type":"event","event":"initialized"})).unwrap()
        else {
            panic!()
        };

        let Incoming::Event(Event::Known(DapEvent::Stopped(s))) = classify(json!({
            "seq":1,"type":"event","event":"stopped",
            "body":{"reason":"breakpoint","threadId":3,"allThreadsStopped":true}
        }))
        .unwrap() else {
            panic!()
        };
        assert_eq!(s.reason, "breakpoint");
        assert_eq!(s.thread_id, serde_json::from_value(json!(3)).ok());

        let Incoming::Event(Event::Known(DapEvent::Output(o))) = classify(json!({
            "seq":1,"type":"event","event":"output","body":{"output":"hi","category":"stderr"}
        }))
        .unwrap() else { panic!() };
        assert_eq!(o.category.as_deref(), Some("stderr"));

        let Incoming::Event(Event::Unknown { name, body }) = classify(json!({
            "seq":1,"type":"event","event":"vendor/thing","body":{"k":1}
        }))
        .unwrap() else {
            panic!()
        };
        assert_eq!((name.as_str(), body), ("vendor/thing", json!({"k":1})));

        let Incoming::Request(req) = classify(json!({
            "seq":9,"type":"request","command":"runInTerminal","arguments":{"args":["a"]}
        }))
        .unwrap() else { panic!() };
        assert_eq!(req.command, "runInTerminal");

        assert!(classify(json!({"type":"nope"})).is_err());
        assert!(classify(json!({"seq":1})).is_err());
    }

    fn response(request_seq: u64, success: bool) -> DapResponse {
        DapResponse {
            seq: 100 + request_seq,
            request_seq,
            success,
            command: "x".into(),
            message: (!success).then(|| "nope".to_string()),
            body: Some(json!({"n": request_seq})),
        }
    }

    #[test]
    fn pending_matches_out_of_order_and_fails_on_close() {
        let pending = Pending::new();
        let (a, rx_a) = pending.register().unwrap();
        let (b, rx_b) = pending.register().unwrap();
        let (c, rx_c) = pending.register().unwrap();
        assert_eq!((a, b, c), (1, 2, 3));
        assert!(pending.resolve(response(b, true)));
        assert!(pending.resolve(response(a, false)));
        assert!(!pending.resolve(response(77, true)), "unknown seq");
        assert!(!pending.resolve(response(a, true)), "already resolved");
        assert_eq!(rx_b.recv().unwrap().unwrap().body.unwrap()["n"], 2);
        let err = wait_response(&pending, a, "cmd", &rx_a, Duration::from_secs(1))
            .unwrap_err();
        assert!(err.to_string().contains("cmd failed: nope"), "{err}");
        assert_eq!(pending.len(), 1);
        pending.fail_all("gone");
        assert!(rx_c.recv().unwrap().unwrap_err().contains("gone"));
        assert!(pending.is_empty());
        assert!(pending.register().is_err(), "closed table refuses new work");
    }

    #[test]
    fn wait_response_times_out_and_forgets() {
        let pending = Pending::new();
        let (seq, rx) = pending.register().unwrap();
        let err =
            wait_response(&pending, seq, "slow", &rx, Duration::from_millis(30))
                .unwrap_err();
        assert!(err.to_string().contains("timed out"));
        assert!(pending.is_empty());
    }

    #[test]
    fn pump_resolves_responses_and_queues_events_then_closes() {
        let pending = Arc::new(Pending::new());
        let (seq, rx) = pending.register().unwrap();
        let (_seq2, rx2) = pending.register().unwrap();
        let mut bytes = Vec::new();
        bytes.extend(frame(
            &json!({"seq":1,"type":"event","event":"initialized"}),
        ));
        bytes.extend(frame(&json!({
            "seq":2,"type":"response","request_seq":seq,"success":true,"command":"c"
        })));
        bytes.extend(frame(
            &json!({"seq":3,"type":"request","command":"runInTerminal"}),
        ));
        bytes.extend(b"garbage-without-colon\r\n\r\n");
        let (tx, inbound) = unbounded();
        pump(Cursor::new(bytes), &pending, &tx);
        assert!(rx.recv().unwrap().unwrap().success);
        assert!(
            rx2.recv().unwrap().is_err(),
            "unanswered request fails on close"
        );
        assert!(matches!(
            inbound.recv().unwrap(),
            Inbound::Event(Event::Known(DapEvent::Initialized(_)))
        ));
        assert!(
            matches!(inbound.recv().unwrap(), Inbound::Request(r) if r.command == "runInTerminal")
        );
        assert!(
            matches!(inbound.recv().unwrap(), Inbound::Closed(reason) if reason.contains("protocol error"))
        );
    }

    #[test]
    fn transport_writes_sequenced_requests() {
        #[derive(Clone, Default)]
        struct Sink(Arc<Mutex<Vec<u8>>>);
        impl Write for Sink {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let sink = Sink::default();
        let transport =
            Transport::new(Box::new(sink.clone()), Arc::new(Pending::new()));
        transport.send_request("threads", None).unwrap();
        transport
            .send_request("next", Some(json!({"threadId":1})))
            .unwrap();
        let mut cursor = Cursor::new(sink.0.lock().clone());
        let first = read_message(&mut cursor).unwrap().unwrap();
        let second = read_message(&mut cursor).unwrap().unwrap();
        assert_eq!(first, json!({"seq":1,"type":"request","command":"threads"}));
        assert_eq!(second["seq"], 2);
        assert_eq!(second["arguments"]["threadId"], 1);
    }

    #[test]
    fn launch_arguments_merge_adapter_extras() {
        let config = RunDebugConfig {
            ty: Some("php".into()),
            name: "run".into(),
            program: "index.php".into(),
            args: Some(vec!["-v".into()]),
            cwd: Some("/w".into()),
            env: None,
            prelaunch: None,
            debug_command: Some(vec!["adapter".into(), "--stdio".into()]),
            dap_id: DapId::default(),
            tracing_output: false,
            config_source: Default::default(),
        };
        let mut spec = default_adapter(&config).unwrap();
        assert_eq!(
            (spec.program.as_str(), spec.args.clone()),
            ("adapter", vec!["--stdio".to_string()])
        );
        spec.extra_arguments = Some(json!({"port": 9003, "name": "override"}));
        let args = launch_arguments(&config, &spec);
        assert_eq!(args["request"], "launch");
        assert_eq!(args["program"], "index.php");
        assert_eq!(args["port"], 9003);
        assert_eq!(args["name"], "override");
        let mut none = config.clone();
        none.debug_command = None;
        assert!(default_adapter(&none).is_none());
    }

    fn mock_spec() -> AdapterSpec {
        AdapterSpec {
            program: "python3".into(),
            args: vec![
                concat!(env!("CARGO_MANIFEST_DIR"), "/src/dap/mock_adapter.py")
                    .into(),
            ],
            adapter_id: "mock".into(),
            ..Default::default()
        }
    }

    fn mock_config(program: &str) -> RunDebugConfig {
        RunDebugConfig {
            ty: Some("mock".into()),
            name: "mock".into(),
            program: program.into(),
            args: None,
            cwd: None,
            env: None,
            prelaunch: None,
            debug_command: None,
            dap_id: DapId::default(),
            tracing_output: false,
            config_source: Default::default(),
        }
    }

    fn next_notification(
        core: &CoreRpcHandler,
        pred: impl Fn(&CoreNotification) -> bool,
    ) -> CoreNotification {
        loop {
            match core
                .rx()
                .recv_timeout(Duration::from_secs(20))
                .expect("core notification")
            {
                CoreRpc::Notification(n) if pred(&n) => return *n,
                _ => {}
            }
        }
    }

    #[test]
    fn mock_adapter_full_session() {
        let core = CoreRpcHandler::new();
        let config = mock_config("prog");
        let dap_id = config.dap_id;
        let spec = mock_spec();
        let manager = Manager::with_resolver(core.clone(), {
            let spec = spec.clone();
            Arc::new(move |_| Some(spec.clone()))
        });
        let path = PathBuf::from("/tmp/a.php");
        let breakpoints = HashMap::from([(
            path.clone(),
            vec![SourceBreakpoint {
                line: 3,
                ..Default::default()
            }],
        )]);
        manager.start(config, breakpoints, None);

        let CoreNotification::DapBreakpointsResp {
            path: p,
            breakpoints,
            ..
        } = next_notification(&core, |n| {
            matches!(n, CoreNotification::DapBreakpointsResp { .. })
        })
        else {
            unreachable!()
        };
        assert_eq!(p, path);
        assert!(breakpoints[0].verified && breakpoints[0].line == Some(3));

        let CoreNotification::Log { message, .. } = next_notification(
            &core,
            |n| matches!(n, CoreNotification::Log { message, target, .. } if target.as_deref()==Some("dap") && message.starts_with("hello")),
        ) else {
            unreachable!()
        };
        assert_eq!(message, "hello\n");

        let CoreNotification::DapStopped {
            dap_id: id,
            stopped,
            stack_frames,
            variables,
        } = next_notification(&core, |n| {
            matches!(n, CoreNotification::DapStopped { .. })
        })
        else {
            unreachable!()
        };
        assert_eq!(id, dap_id);
        assert_eq!(stopped.reason, "breakpoint");
        let frames = stack_frames.values().next().unwrap();
        assert_eq!((frames[0].id, frames[0].line), (10, 3));
        assert_eq!(variables[0].0.name, "Locals");
        assert_eq!(variables[0].1[0].name, "$a");

        // request helpers
        assert_eq!(manager.variables(dap_id, 100).unwrap()[0].value, "1");
        assert_eq!(manager.scopes(dap_id, 10).unwrap()[0].1[0].name, "$a");

        let thread = stopped_thread(&stack_frames);
        manager.continue_(dap_id, thread);
        next_notification(&core, |n| {
            matches!(n, CoreNotification::DapContinued { .. })
        });
        manager.step_over(dap_id, thread);
        manager.step_into(dap_id, thread);
        manager.step_out(dap_id, thread);
        manager.pause(dap_id, thread);

        manager.stop(dap_id);
        next_notification(&core, |n| {
            matches!(n, CoreNotification::DapContinued { .. })
        });
        assert!(manager.session(dap_id).is_none());
    }

    fn stopped_thread(
        frames: &HashMap<ThreadId, Vec<dap_types::StackFrame>>,
    ) -> ThreadId {
        *frames.keys().next().unwrap()
    }

    #[test]
    fn launch_failure_is_reported_and_session_removed() {
        let core = CoreRpcHandler::new();
        let config = mock_config("fail");
        let dap_id = config.dap_id;
        let spec = mock_spec();
        let manager = Manager::with_resolver(
            core.clone(),
            Arc::new(move |_| Some(spec.clone())),
        );
        manager.start(config, HashMap::new(), None);
        let CoreNotification::Log { message, level, .. } = next_notification(
            &core,
            |n| matches!(n, CoreNotification::Log { message, .. } if message.contains("Failed to start debugging")),
        ) else {
            unreachable!()
        };
        assert!(message.contains("boom"), "{message}");
        assert!(matches!(level, LogLevel::Error));
        assert!(manager.session(dap_id).is_none());
    }

    #[test]
    fn missing_adapter_is_reported() {
        let core = CoreRpcHandler::new();
        let manager = Manager::new(core.clone());
        manager.start(mock_config("x"), HashMap::new(), None);
        let CoreNotification::Log { message, .. } =
            next_notification(&core, |n| matches!(n, CoreNotification::Log { .. }))
        else {
            unreachable!()
        };
        assert!(message.contains("No debug adapter"));
    }

    #[test]
    fn unsupported_reverse_request_gets_error_response() {
        let (adapter_in, to_adapter) = std::io::pipe().unwrap();
        let (adapter_out, from_adapter) = std::io::pipe().unwrap();
        let core = CoreRpcHandler::new();
        let _session = Session::from_io(
            DapId::default(),
            mock_config("x"),
            Box::new(to_adapter),
            Box::new(BufReader::new(adapter_out)),
            core,
        );
        let mut adapter_in = BufReader::new(adapter_in);
        let mut from_adapter = from_adapter;
        write_message(&mut from_adapter, &json!({"seq":7,"type":"request","command":"runInTerminal","arguments":{}})).unwrap();
        let reply = read_message(&mut adapter_in).unwrap().unwrap();
        assert_eq!(reply["type"], "response");
        assert_eq!(reply["request_seq"], 7);
        assert_eq!(reply["success"], false);
    }
    #[test]
    fn listen_arguments_omit_program_and_keep_debuggee_env() {
        let mut config = mock_config("");
        config.env = Some(HashMap::from([("XDEBUG_MODE".into(), "debug".into())]));
        let spec = mock_spec();
        let args = launch_arguments(&config, &spec);
        assert!(args.get("program").is_none());
        assert_eq!(args["request"], "launch");
        assert_eq!(args["env"]["XDEBUG_MODE"], "debug");
        assert!(spec.env.is_empty());
    }

    #[test]
    fn early_launch_response_still_sets_breakpoints() {
        let core = CoreRpcHandler::new();
        let spec = mock_spec();
        let session = Session::spawn(
            DapId::default(),
            mock_config("early"),
            &spec,
            core.clone(),
        )
        .unwrap();
        session
            .handshake(
                &spec,
                &HashMap::from([(
                    PathBuf::from("/tmp/a.php"),
                    vec![SourceBreakpoint {
                        line: 3,
                        ..Default::default()
                    }],
                )]),
            )
            .unwrap();
        next_notification(&core, |n| {
            matches!(n, CoreNotification::DapBreakpointsResp { .. })
        });
        session.shutdown();
    }

    #[test]
    fn terminated_event_does_not_skip_process_cleanup() {
        let spec = mock_spec();
        let session = Session::spawn(
            DapId::default(),
            mock_config("x"),
            &spec,
            CoreRpcHandler::new(),
        )
        .unwrap();
        session.handle_event(Event::Known(DapEvent::Terminated(None)));
        assert!(session.is_ended());
        session.shutdown();
        assert!(session.child.lock().is_none());
        session.shutdown();
    }

    #[test]
    fn standard_dap_server_conversion_preserves_spawn_fields() {
        let spec = AdapterSpec::from_dap_server(
            dap_types::DapServer {
                program: "node".into(),
                args: vec!["adapter.js".into()],
                cwd: Some(PathBuf::from("/w")),
            },
            "php".into(),
        );
        assert_eq!(spec.program, "node");
        assert_eq!(spec.args, vec!["adapter.js"]);
        assert_eq!(spec.cwd, Some(PathBuf::from("/w")));
        assert_eq!(spec.request, RequestKind::Launch);
    }
    #[test]
    fn shutdown_cancels_pending_start_reservations() {
        let manager = Manager::new(CoreRpcHandler::new());
        let id = DapId::default();
        manager.starts.lock().insert(id, 42);
        manager.shutdown_all();
        assert!(manager.starts.lock().is_empty());
        manager.starts.lock().insert(id, 43);
        manager.stop(id);
        assert!(manager.starts.lock().is_empty());
    }
}
