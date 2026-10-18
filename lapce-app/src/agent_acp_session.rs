//! ACP session controller: the handshake and prompt-turn state machine
//! for one hosted agent (opencode natively, Claude Code/Codex via their
//! adapters). Pure logic: the caller owns the `AcpProcess`, shuttles the
//! `Value`s this module emits onto the wire, and feeds every decoded
//! `AcpMessage` back into `handle`. No subprocess, filesystem walk or UI
//! of its own - the only filesystem touch is answering the agent's
//! `fs/read_text_file` requests, bounded to the canonical workspace root
//! with a 1 MiB cap and no symlink escape. Writes are refused with a
//! JSON-RPC error until the review/permission UI owns them; terminal
//! capability is never advertised here.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::agent_acp::{self, AcpError, AcpMessage, FsRead, FsWrite};
use crate::agent_acp_process::next_id;

/// Cap on bytes read when answering `fs/read_text_file`.
const MAX_READ: u64 = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// `initialize` sent, hello not yet accepted (or auth in flight).
    Initializing,
    /// Hello accepted, `session/new` in flight.
    Starting,
    /// Session id held, idle between turns.
    Ready,
    /// A `session/prompt` is in flight.
    Prompting,
    /// Unrecoverable protocol failure; kill the process.
    Closed,
}

/// One UI-facing thing that happened. The chat panel renders these in
/// arrival order; nothing here is Lapce-specific.
#[derive(Clone, Debug, PartialEq)]
pub enum TurnEvent {
    Connected { agent: String, version: String, load_session: bool },
    /// The agent requires authentication before `session/new`.
    NeedsAuth { methods: Vec<String> },
    SessionStarted { session_id: String },
    Text { text: String },
    Plan { entries: Vec<String> },
    ToolCall { id: String, title: String, status: String },
    Usage { used: i64, size: i64 },
    /// An fs/read request was answered (`ok` false = refused with error).
    FsRead { path: String, ok: bool },
    /// Something tolerable and unhandled (unknown update kind, etc).
    Notice { text: String },
    Error { message: String },
    TurnDone { stop_reason: String },
    Closed,
}

pub struct AcpSession {
    phase: Phase,
    workspace: PathBuf,
    /// Agent file writes staged for owner review: (canonical path,
    /// content), last write per path wins. Drained at turn end and
    /// handed to the review panel; never written to disk or buffers
    /// by the session itself.
    staged_writes: Vec<(String, String)>,
    session_id: Option<String>,
    init_id: i64,
    auth_id: Option<i64>,
    new_id: Option<i64>,
    prompt_id: Option<i64>,
    /// Resume this stored session instead of starting a new one.
    resume_id: Option<String>,
    /// The agent's loadSession capability, from its hello.
    can_load: bool,
}

fn err(code: i64, message: impl Into<String>) -> AcpError {
    AcpError { code, message: message.into() }
}

fn result_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

impl AcpSession {
    /// Begin the handshake. `workspace` MUST already be canonicalized -
    /// fs answers are confined to it by prefix comparison.
    pub fn start(workspace: PathBuf, client_name: &str, client_version: &str) -> (Self, Value) {
        Self::start_resumable(workspace, client_name, client_version, None)
    }

    /// Begin the handshake, resuming `resume` (a stored session id)
    /// instead of creating a fresh session when the agent advertises
    /// loadSession. Agents without the capability get a Notice and a
    /// fresh session instead (spec forbids calling session/load).
    pub fn start_resumable(
        workspace: PathBuf,
        client_name: &str,
        client_version: &str,
        resume: Option<String>,
    ) -> (Self, Value) {
        let init_id = next_id();
        // Advertise fs read AND write: writes are staged for owner
        // review (never applied directly), so the capability is
        // honest - the agent's writes do land, via the review panel.
        let mut hello = agent_acp::initialize(init_id, client_name, client_version, true, false);
        if let Some(w) = hello.pointer_mut("/params/clientCapabilities/fs/writeTextFile") {
            *w = Value::Bool(true);
        }
        let session = AcpSession {
            phase: Phase::Initializing,
            workspace,
            staged_writes: Vec::new(),
            session_id: None,
            init_id,
            auth_id: None,
            new_id: None,
            prompt_id: None,
            resume_id: resume,
            can_load: false,
        };
        (session, hello)
    }

    /// The agent's loadSession capability as negotiated.
    pub fn can_resume(&self) -> bool {
        self.can_load
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// After `NeedsAuth`, run one advertised method. On success the
    /// controller proceeds to `session/new` by itself.
    pub fn begin_auth(&mut self, method_id: &str) -> Result<Value, AcpError> {
        if self.phase != Phase::Initializing {
            return Err(err(-32002, "agent is not awaiting authentication"));
        }
        let id = next_id();
        self.auth_id = Some(id);
        Ok(agent_acp::authenticate(id, method_id))
    }

    /// User pressed send. Only valid while `Ready`.
    pub fn begin_prompt(&mut self, text: &str) -> Result<Value, AcpError> {
        if self.phase != Phase::Ready {
            return Err(err(-32002, "agent is not ready for a prompt"));
        }
        let session_id =
            self.session_id.clone().ok_or_else(|| err(-32002, "no session"))?;
        let id = next_id();
        self.prompt_id = Some(id);
        self.phase = Phase::Prompting;
        Ok(agent_acp::session_prompt(id, &session_id, text))
    }

    /// Stop button. Only meaningful mid-turn; the agent may still finish
    /// the turn and answer the prompt request.
    pub fn cancel(&self) -> Option<Value> {
        if self.phase == Phase::Prompting {
            self.session_id.as_deref().map(agent_acp::session_cancel)
        } else {
            None
        }
    }

    /// Feed one decoded inbound message. Returns UI events plus outbound
    /// messages (request responses, follow-up requests) for the wire.
    pub fn handle(&mut self, message: &AcpMessage) -> (Vec<TurnEvent>, Vec<Value>) {
        let mut events = Vec::new();
        let mut out = Vec::new();
        match message {
            AcpMessage::Response { id, result, error } => {
                let numeric = id.as_i64();
                if let Some(e) = error {
                    events.push(TurnEvent::Error { message: e.message.clone() });
                    if numeric == self.prompt_id {
                        self.prompt_id = None;
                        self.phase = Phase::Ready;
                    } else if numeric == Some(self.init_id)
                        || numeric == self.auth_id
                        || numeric == self.new_id
                    {
                        self.phase = Phase::Closed;
                        events.push(TurnEvent::Closed);
                    }
                    return (events, out);
                }
                let result = result.clone().unwrap_or_else(|| json!({}));
                if numeric == Some(self.init_id) || numeric == self.auth_id {
                    self.on_hello(numeric == Some(self.init_id), &result, &mut events, &mut out);
                } else if numeric == self.new_id {
                    // A session/load result may carry no sessionId; the id
                    // we resumed is the session. session/new MUST have one.
                    let sid = agent_acp::parse_session_id(&result)
                        .or_else(|| self.resume_id.clone());
                    match sid {
                        Some(sid) => {
                            self.session_id = Some(sid.clone());
                            self.phase = Phase::Ready;
                            events.push(TurnEvent::SessionStarted { session_id: sid });
                        }
                        None => {
                            events.push(TurnEvent::Error {
                                message: "session/new result had no sessionId".into(),
                            });
                            self.phase = Phase::Closed;
                            events.push(TurnEvent::Closed);
                        }
                    }
                } else if numeric == self.prompt_id {
                    self.prompt_id = None;
                    self.phase = Phase::Ready;
                    let reason =
                        agent_acp::parse_stop_reason(&result).unwrap_or_else(|| "end_turn".into());
                    events.push(TurnEvent::TurnDone { stop_reason: reason });
                } else {
                    events.push(TurnEvent::Notice { text: "unmatched response id".into() });
                }
            }
            AcpMessage::Notification { method, params } => {
                if method == "session/update" {
                    self.on_update(params, &mut events);
                } else {
                    events.push(TurnEvent::Notice {
                        text: format!("unhandled notification {method}"),
                    });
                }
            }
            AcpMessage::Request { id, method, params } => match method.as_str() {
                "fs/read_text_file" => match agent_acp::parse_fs_read(params) {
                    Some(read) => {
                        let (reply, event) = self.answer_fs_read(id, &read);
                        out.push(reply);
                        events.push(event);
                    }
                    None => {
                        out.push(error_response(id, -32602, "bad fs/read_text_file params"));
                    }
                },
                "fs/write_text_file" => match agent_acp::parse_fs_write(params) {
                    Some(write) => {
                        let (reply, event) = self.stage_fs_write(id, &write);
                        out.push(reply);
                        events.push(event);
                    }
                    None => {
                        out.push(error_response(id, -32602, "bad fs/write_text_file params"));
                    }
                },
                other => {
                    out.push(error_response(
                        id,
                        -32601,
                        &format!("unsupported method {other}"),
                    ));
                }
            },
        }
        (events, out)
    }

    fn on_hello(
        &mut self,
        is_init: bool,
        result: &Value,
        events: &mut Vec<TurnEvent>,
        out: &mut Vec<Value>,
    ) {
        if is_init {
            match agent_acp::parse_hello(result) {
                Ok(hello) => {
                    if hello.protocol_version != 1 {
                        events.push(TurnEvent::Error {
                            message: format!(
                                "unsupported protocol version {}",
                                hello.protocol_version
                            ),
                        });
                        self.phase = Phase::Closed;
                        events.push(TurnEvent::Closed);
                        return;
                    }
                    self.can_load = hello.load_session;
                    events.push(TurnEvent::Connected {
                        agent: hello.name.clone(),
                        version: hello.version.clone(),
                        load_session: hello.load_session,
                    });
                    if self.resume_id.is_some() && !hello.load_session {
                        events.push(TurnEvent::Notice {
                            text: "agent cannot load sessions; starting a fresh one".into(),
                        });
                        self.resume_id = None;
                    }
                    if !hello.auth_methods.is_empty() {
                        events.push(TurnEvent::NeedsAuth { methods: hello.auth_methods });
                        return; // stay Initializing until begin_auth answers
                    }
                    self.begin_session(events, out);
                }
                Err(e) => {
                    events.push(TurnEvent::Error { message: e.message });
                    self.phase = Phase::Closed;
                    events.push(TurnEvent::Closed);
                }
            }
        } else {
            // An authenticate response carries no meaningful result body.
            self.auth_id = None;
            self.begin_session(events, out);
        }
    }

    fn begin_session(&mut self, _events: &mut [TurnEvent], out: &mut Vec<Value>) {
        let id = next_id();
        self.new_id = Some(id);
        self.phase = Phase::Starting;
        let cwd = self.workspace.to_string_lossy().into_owned();
        match &self.resume_id {
            Some(sid) => out.push(agent_acp::session_load(id, sid, &cwd)),
            None => out.push(agent_acp::session_new(id, &cwd)),
        }
    }

    fn on_update(&self, params: &Value, events: &mut Vec<TurnEvent>) {
        match agent_acp::parse_session_update(params) {
            Some((sid, update)) => {
                if Some(&sid) != self.session_id.as_ref() {
                    events.push(TurnEvent::Notice { text: "update for unknown session".into() });
                    return;
                }
                match update {
                    agent_acp::SessionUpdate::AgentChunk { text } => {
                        events.push(TurnEvent::Text { text });
                    }
                    agent_acp::SessionUpdate::Plan { entries } => {
                        events.push(TurnEvent::Plan { entries });
                    }
                    agent_acp::SessionUpdate::ToolCall { id, title, status } => {
                        events.push(TurnEvent::ToolCall { id, title, status });
                    }
                    agent_acp::SessionUpdate::Usage { used, size } => {
                        events.push(TurnEvent::Usage { used, size });
                    }
                    agent_acp::SessionUpdate::Other { kind } => {
                        events.push(TurnEvent::Notice {
                            text: format!("unhandled update kind {kind}"),
                        });
                    }
                }
            }
            None => events.push(TurnEvent::Notice { text: "malformed session/update".into() }),
        }
    }

    /// Stage an agent file write for owner review instead of writing
    /// it: confined, existing files only; last write per path wins.
    /// The agent gets success - the write IS accepted, into the
    /// review panel, which owns actually applying it.
    fn stage_fs_write(&mut self, id: &Value, write: &FsWrite) -> (Value, TurnEvent) {
        let candidate = {
            let p = Path::new(&write.path);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                self.workspace.join(p)
            }
        };
        let canonical = match candidate.canonicalize() {
            Ok(c) => c,
            Err(e) => {
                return (
                    error_response(
                        id,
                        -32602,
                        &format!("cannot stage write to {}: {e}", write.path),
                    ),
                    TurnEvent::Notice {
                        text: format!(
                            "refused a write to {}: not an existing file",
                            write.path
                        ),
                    },
                );
            }
        };
        if !canonical.starts_with(&self.workspace) {
            return (
                error_response(
                    id,
                    -32602,
                    &format!("{} is outside the workspace", write.path),
                ),
                TurnEvent::Notice {
                    text: format!(
                        "refused an out-of-workspace write to {}",
                        write.path
                    ),
                },
            );
        }
        if write.content.len() > 2 * 1024 * 1024 {
            return (
                error_response(id, -32602, "write exceeds the 2 MiB review limit"),
                TurnEvent::Notice {
                    text: format!("refused an oversized write to {}", write.path),
                },
            );
        }
        let path = canonical.to_string_lossy().into_owned();
        if let Some(entry) =
            self.staged_writes.iter_mut().find(|(p, _)| *p == path)
        {
            entry.1 = write.content.clone();
        } else {
            if self.staged_writes.len() >= 32 {
                return (
                    error_response(id, -32602, "too many staged writes (32 max)"),
                    TurnEvent::Notice {
                        text: "refused a write past the 32-file review limit"
                            .into(),
                    },
                );
            }
            self.staged_writes.push((path.clone(), write.content.clone()));
        }
        (
            result_response(id.clone(), Value::Null),
            TurnEvent::Notice {
                text: format!("staged a write to {path} for owner review"),
            },
        )
    }

    /// Drain writes staged during the turn: (canonical path, content).
    pub fn take_staged_writes(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.staged_writes)
    }

    fn answer_fs_read(&self, id: &Value, read: &FsRead) -> (Value, TurnEvent) {
        match read_within(&self.workspace, &read.path, read.line, read.limit) {
            Ok(content) => (
                result_response(id.clone(), json!({ "content": content })),
                TurnEvent::FsRead { path: read.path.clone(), ok: true },
            ),
            Err(e) => (
                error_response(id, e.code, &e.message),
                TurnEvent::FsRead { path: read.path.clone(), ok: false },
            ),
        }
    }
}

/// Read `path` (absolute or root-relative) confined to the canonical
/// `root`. `line` is 1-indexed, `limit` a line count, per ACP. Overlong
/// files are capped at 1 MiB BEFORE line slicing.
fn read_within(
    root: &Path,
    path: &str,
    line: Option<u64>,
    limit: Option<u64>,
) -> Result<String, AcpError> {
    use std::io::Read as _;

    let candidate = {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            root.join(p)
        }
    };
    let canonical = candidate
        .canonicalize()
        .map_err(|e| err(-32602, format!("cannot read {path}: {e}")))?;
    if !canonical.starts_with(root) {
        return Err(err(-32602, format!("{path} is outside the workspace")));
    }
    let file = std::fs::File::open(&canonical)
        .map_err(|e| err(-32603, format!("cannot open {path}: {e}")))?;
    let mut buf = Vec::new();
    file.take(MAX_READ + 1)
        .read_to_end(&mut buf)
        .map_err(|e| err(-32603, format!("cannot read {path}: {e}")))?;
    buf.truncate(MAX_READ as usize);
    // Invalid UTF-8 is replaced rather than failing the agent's turn.
    let text = String::from_utf8_lossy(&buf).into_owned();
    if line.is_none() && limit.is_none() {
        return Ok(text);
    }
    let lines: Vec<&str> = text.lines().collect();
    let start = line.unwrap_or(1).max(1) as usize - 1;
    if start >= lines.len() {
        return Ok(String::new());
    }
    let end = match limit {
        Some(n) => start.saturating_add(n as usize).min(lines.len()),
        None => lines.len(),
    };
    Ok(lines[start..end].join("\n"))
}

/// Build the `session/prompt` text with the editor context attached,
/// mirroring chat::assemble's wrapping so the untrusted-data fence is
/// identical across every AI surface. The context (12 KB capped by
/// chat::Context) leaves the editor: the agent subprocess forwards it to
/// whatever model IT talks to, which the panel's per-Send indicator and
/// turn context line must make visible.
pub fn compose_prompt(context: Option<&crate::chat::Context>, prompt: &str) -> String {
    match context {
        Some(c) => format!(
            "{prompt}\n\nEditor context ({})\n<editor_context>\n{}\n</editor_context>",
            c.indicator(),
            c.text
        ),
        None => prompt.to_string(),
    }
}

/// Split a configured agent command (`opencode acp`, `codex-acp --flag`)
/// into program + args. Whitespace only, no quoting or escaping: keep
/// the binary path space-free, which package managers already do.
pub fn split_command(spec: &str) -> Option<(String, Vec<String>)> {
    let mut parts = spec.split_whitespace();
    let program = parts.next()?.to_string();
    Some((program, parts.map(str::to_string).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(id: i64, result: Value) -> AcpMessage {
        AcpMessage::Response { id: json!(id), result: Some(result), error: None }
    }

    fn notification(method: &str, params: Value) -> AcpMessage {
        AcpMessage::Notification { method: method.to_string(), params }
    }

    fn request(id: i64, method: &str, params: Value) -> AcpMessage {
        AcpMessage::Request {
            id: json!(id),
            method: method.to_string(),
            params,
        }
    }

    fn write_request(id: i64, path: &str, content: &str) -> AcpMessage {
        request(
            id,
            "fs/write_text_file",
            json!({"sessionId": "s", "path": path, "content": content}),
        )
    }

    #[test]
    fn writes_stage_for_review_and_last_write_wins() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "old").unwrap();
        let (mut session, hello) =
            AcpSession::start_resumable(dir.path().to_path_buf(), "t", "1", None);
        // Write capability is advertised.
        assert_eq!(
            hello.pointer("/params/clientCapabilities/fs/writeTextFile"),
            Some(&Value::Bool(true))
        );

        let (events, out) = session.handle(&write_request(
            7,
            dir.path().join("a.txt").to_str().unwrap(),
            "v1",
        ));
        assert!(out[0].pointer("/result").is_some());
        assert!(matches!(events[0], TurnEvent::Notice { .. }));
        // Disk is untouched: staging is not applying.
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "old"
        );

        let _ = session.handle(&write_request(
            8,
            dir.path().join("a.txt").to_str().unwrap(),
            "v2",
        ));
        let staged = session.take_staged_writes();
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].1, "v2");
        // Drained.
        assert!(session.take_staged_writes().is_empty());
    }

    #[test]
    fn writes_refuse_outside_missing_and_oversized() {
        let dir = tempfile::tempdir().unwrap();
        let (mut session, _) =
            AcpSession::start_resumable(dir.path().to_path_buf(), "t", "1", None);

        let (_events, out) =
            session.handle(&write_request(1, "/etc/hostname", "x"));
        assert!(out[0].pointer("/error").is_some());

        let missing = dir.path().join("new-file.txt");
        let (_events, out) =
            session.handle(&write_request(2, missing.to_str().unwrap(), "x"));
        assert!(out[0].pointer("/error").is_some());

        std::fs::write(dir.path().join("a.txt"), "old").unwrap();
        let huge = "x".repeat(2 * 1024 * 1024 + 1);
        let (_events, out) = session.handle(&write_request(
            3,
            dir.path().join("a.txt").to_str().unwrap(),
            &huge,
        ));
        assert!(out[0].pointer("/error").is_some());
        assert!(session.take_staged_writes().is_empty());
    }

    fn hello_result(auth: bool) -> Value {
        json!({
            "protocolVersion": 1,
            "agentInfo": { "name": "opencode", "version": "0.6.3" },
            "authMethods": if auth { json!([{ "id": "oauth" }]) } else { json!([]) },
            "agentCapabilities": { "loadSession": false },
        })
    }

    fn temp_workspace(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("acp-session-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    /// Drive the no-auth handshake and return a Ready session.
    fn ready_session(tag: &str) -> AcpSession {
        let root = temp_workspace(tag);
        let (mut session, init) = AcpSession::start(root, "lapce", "0.1");
        assert_eq!(init["method"], json!("initialize"));
        let init_id = init["id"].as_i64().unwrap();
        let (events, out) = session.handle(&response(init_id, hello_result(false)));
        assert_eq!(events.len(), 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["method"], json!("session/new"));
        let new_id = out[0]["id"].as_i64().unwrap();
        let (events, _) = session.handle(&response(new_id, json!({ "sessionId": "s-1" })));
        assert_eq!(events.len(), 1);
        assert_eq!(session.phase(), Phase::Ready);
        session
    }

    #[test]
    fn handshake_reaches_ready() {
        let session = ready_session("handshake");
        assert_eq!(session.session_id(), Some("s-1"));
    }

    #[test]
    fn hello_with_auth_waits_then_proceeds() {
        let root = temp_workspace("auth");
        let (mut session, init) = AcpSession::start(root, "lapce", "0.1");
        let init_id = init["id"].as_i64().unwrap();
        let (events, out) = session.handle(&response(init_id, hello_result(true)));
        assert_eq!(events.len(), 2);
        assert!(out.is_empty());
        assert_eq!(session.phase(), Phase::Initializing);
        let auth = session.begin_auth("oauth").unwrap();
        assert_eq!(auth["method"], json!("authenticate"));
        let auth_id = auth["id"].as_i64().unwrap();
        let (_, out) = session.handle(&response(auth_id, json!({})));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["method"], json!("session/new"));
        assert_eq!(session.phase(), Phase::Starting);
    }

    #[test]
    fn prompt_turn_streams_and_finishes() {
        let mut session = ready_session("turn");
        let prompt = session.begin_prompt("fix the router").unwrap();
        assert_eq!(prompt["method"], json!("session/prompt"));
        assert_eq!(prompt["params"]["sessionId"], json!("s-1"));
        assert_eq!(session.phase(), Phase::Prompting);

        let chunk = notification(
            "session/update",
            json!({
                "sessionId": "s-1",
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": { "type": "text", "text": "héllo wörld" }
                }
            }),
        );
        let (events, out) = session.handle(&chunk);
        assert!(out.is_empty());
        let expected_chunk = TurnEvent::Text { text: "héllo wörld".into() };
        assert_eq!(events[0], expected_chunk);

        let plan = notification(
            "session/update",
            json!({
                "sessionId": "s-1",
                "update": {
                    "sessionUpdate": "plan",
                    "entries": [{ "content": "read routes", "status": "in_progress" }]
                }
            }),
        );
        let (events, _) = session.handle(&plan);
        let expected_plan = TurnEvent::Plan { entries: vec!["read routes (in_progress)".into()] };
        assert_eq!(events[0], expected_plan);

        let prompt_id = prompt["id"].as_i64().unwrap();
        let (events, _) = session.handle(&response(prompt_id, json!({ "stopReason": "end_turn" })));
        let expected_done = TurnEvent::TurnDone { stop_reason: "end_turn".into() };
        assert_eq!(events[0], expected_done);
        assert_eq!(session.phase(), Phase::Ready);
    }

    #[test]
    fn cancel_only_mid_turn() {
        let mut session = ready_session("cancel");
        assert!(session.cancel().is_none());
        let _ = session.begin_prompt("go").unwrap();
        let cancel = session.cancel().unwrap();
        assert_eq!(cancel["method"], json!("session/cancel"));
        assert_eq!(cancel["params"]["sessionId"], json!("s-1"));
    }

    #[test]
    fn prompt_rejected_when_not_ready() {
        let root = temp_workspace("notready");
        let (mut session, _) = AcpSession::start(root, "lapce", "0.1");
        assert!(session.begin_prompt("too soon").is_err());
    }

    #[test]
    fn fs_read_is_answered_with_line_window() {
        let mut session = ready_session("fsread");
        let root = session.workspace.clone();
        std::fs::write(root.join("routes.php"), "one\ntwó héllo\nthree\nfour\nfive\n").unwrap();
        let req = request(
            9001,
            "fs/read_text_file",
            json!({ "sessionId": "s-1", "path": "routes.php", "line": 2, "limit": 2 }),
        );
        let (events, out) = session.handle(&req);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["result"]["content"], json!("twó héllo\nthree"));
        let expected_fs = TurnEvent::FsRead { path: "routes.php".into(), ok: true };
        assert_eq!(events[0], expected_fs);
    }

    #[test]
    fn fs_read_outside_workspace_is_refused() {
        let mut session = ready_session("fsescape");
        let req = request(
            9002,
            "fs/read_text_file",
            json!({ "sessionId": "s-1", "path": "../../etc/hostname" }),
        );
        let (events, out) = session.handle(&req);
        assert_eq!(out[0]["error"]["code"], json!(-32602));
        let expected_fs = TurnEvent::FsRead { path: "../../etc/hostname".into(), ok: false };
        assert_eq!(events[0], expected_fs);
    }

    #[test]
    fn fs_write_is_refused() {
        // Writes are staged for owner review since the review-lane
        // milestone; a write to a file that does not exist is refused
        // (no implicit creation) and stages nothing.
        let mut session = ready_session("fswrite");
        let req = request(
            9003,
            "fs/write_text_file",
            json!({ "sessionId": "s-1", "path": "x.php", "content": "owned" }),
        );
        let (events, out) = session.handle(&req);
        assert_eq!(out[0]["error"]["code"], json!(-32602));
        assert_eq!(events.len(), 1);
        assert!(session.take_staged_writes().is_empty());
    }

    #[test]
    fn updates_for_other_sessions_are_ignored() {
        let mut session = ready_session("stray");
        let stray = notification(
            "session/update",
            json!({
                "sessionId": "someone-else",
                "update": { "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": "x" } }
            }),
        );
        let (events, _) = session.handle(&stray);
        let expected_notice = TurnEvent::Notice { text: "update for unknown session".into() };
        assert_eq!(events[0], expected_notice);
    }

    #[test]
    fn prompt_error_returns_to_ready() {
        let mut session = ready_session("prompterr");
        let prompt = session.begin_prompt("go").unwrap();
        let prompt_id = prompt["id"].as_i64().unwrap();
        let failure = AcpMessage::Response {
            id: json!(prompt_id),
            result: None,
            error: Some(AcpError { code: -32603, message: "model exploded".into() }),
        };
        let (events, _) = session.handle(&failure);
        let expected_error = TurnEvent::Error { message: "model exploded".into() };
        assert_eq!(events[0], expected_error);
        assert_eq!(session.phase(), Phase::Ready);
    }

    #[test]
    fn fs_read_handles_non_ascii_and_missing_lines() {
        let root = temp_workspace("fuzz");
        let (session, _) = AcpSession::start(root.clone(), "lapce", "0.1");
        let path = root.join("ünïcode.php");
        std::fs::write(&path, "émoji 🦀\n第二行\nümläut\n").unwrap();
        let whole = read_within(&root, "ünïcode.php", None, None).unwrap();
        assert!(whole.contains('🦀'));
        assert!(whole.contains("第二行"));
        let past_end = read_within(&root, "ünïcode.php", Some(99), Some(5)).unwrap();
        assert_eq!(past_end, "");
        let tail = read_within(&root, "ünïcode.php", Some(3), None).unwrap();
        assert_eq!(tail, "ümläut");
        drop(session);
    }

    #[test]
    fn resume_uses_session_load_when_the_agent_can() {
        let root = temp_workspace("resume");
        let (mut session, init) =
            AcpSession::start_resumable(root, "lapce", "0.1", Some("sess-old".into()));
        let init_id = init["id"].as_i64().unwrap();
        let hello = json!({
            "protocolVersion": 1,
            "agentInfo": { "name": "opencode", "version": "0.6.3" },
            "authMethods": [],
            "agentCapabilities": { "loadSession": true },
        });
        let (events, out) = session.handle(&response(init_id, hello));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["method"], json!("session/load"));
        assert_eq!(out[0]["params"]["sessionId"], json!("sess-old"));
        assert!(session.can_resume());
        // A load result may carry no sessionId; the resumed id stands.
        let load_id = out[0]["id"].as_i64().unwrap();
        let (events, _) = session.handle(&response(load_id, json!({})));
        assert_eq!(session.phase(), Phase::Ready);
        assert_eq!(session.session_id(), Some("sess-old"));
        let expected_started =
            TurnEvent::SessionStarted { session_id: "sess-old".into() };
        assert_eq!(events[0], expected_started);
    }

    #[test]
    fn resume_falls_back_when_the_agent_cannot_load() {
        let root = temp_workspace("resumefallback");
        let (mut session, init) =
            AcpSession::start_resumable(root, "lapce", "0.1", Some("sess-old".into()));
        let init_id = init["id"].as_i64().unwrap();
        let (events, out) = session.handle(&response(init_id, hello_result(false)));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["method"], json!("session/new"));
        assert!(!session.can_resume());
        let expected_notice = TurnEvent::Notice {
            text: "agent cannot load sessions; starting a fresh one".into(),
        };
        assert!(events.contains(&expected_notice));
    }

    #[test]
    fn split_command_parses_plain_specs() {
        let parsed = split_command("opencode acp").unwrap();
        assert_eq!(parsed.0, "opencode");
        assert_eq!(parsed.1, vec!["acp".to_string()]);
        let single = split_command("codex-acp").unwrap();
        assert_eq!(single.0, "codex-acp");
        assert!(single.1.is_empty());
        assert!(split_command("   ").is_none());
        let wide = split_command("ägent --fläg").unwrap();
        assert_eq!(wide.0, "ägent");
        assert_eq!(wide.1, vec!["--fläg".to_string()]);
    }

    #[test]
    fn compose_prompt_wraps_context_like_chat() {
        let plain = compose_prompt(None, "héllo");
        assert_eq!(plain, "héllo");
        let context = crate::chat::Context::new("src/röutes.php".into(), "líne 🦀", true);
        let wrapped = compose_prompt(Some(&context), "fix it");
        assert!(wrapped.starts_with("fix it\n\nEditor context (Selection: src/röutes.php"));
        assert!(wrapped.contains("<editor_context>\nlíne 🦀\n</editor_context>"));
        // Same fence as chat::assemble so agents see one convention.
        assert!(wrapped.contains("bytes"));
    }
}
