//! Agent Client Protocol (ACP v1) client foundation: newline-delimited
//! JSON-RPC message shapes for talking to ACP agents (opencode natively;
//! Claude Code and Codex CLI through their ACP adapters). Pure contract:
//! this module does NOT spawn processes, touch stdio, read or write files,
//! apply edits, grant permissions or render UI. Those integrations arrive
//! separately; an agent process must never gain filesystem or execution
//! ability from this module alone.
//!
//! Shapes verified against agentclientprotocol.com/protocol/v1 on
//! 2026-10-17 (transports, initialization, session-setup, prompt-turn,
//! file-system). stdio framing is NDJSON: one JSON-RPC message per line,
//! no embedded newlines.

use serde_json::{Value, json};

/// A decoded inbound message from the agent (or a parse of our own
/// outbound message in tests).
#[derive(Clone, Debug, PartialEq)]
pub enum AcpMessage {
    /// Agent -> client request (expects a response): fs/*, permission.
    Request { id: Value, method: String, params: Value },
    /// Response to a client request (result xor error).
    Response { id: Value, result: Option<Value>, error: Option<AcpError> },
    /// One-way message (session/update).
    Notification { method: String, params: Value },
}

#[derive(Clone, Debug, PartialEq)]
pub struct AcpError {
    pub code: i64,
    pub message: String,
}

/// Encode one message for the wire. serde_json escapes control characters
/// in strings, so the output is always a single line; assert it anyway.
pub fn encode(message: &Value) -> Result<String, AcpError> {
    let mut s = serde_json::to_string(message)
        .map_err(|e| AcpError { code: -32700, message: e.to_string() })?;
    if s.contains('\n') || s.contains('\r') {
        return Err(AcpError {
            code: -32600,
            message: "encoded message contains a newline".into(),
        });
    }
    s.push('\n');
    Ok(s)
}

/// Decode one NDJSON line. A trailing newline is accepted; embedded
/// newline means the caller framed wrongly.
pub fn decode(line: &str) -> Result<AcpMessage, AcpError> {
    let line = line.strip_suffix('\n').unwrap_or(line);
    // Tolerate one CRLF; reject anything embedded after that.
    let line = line.strip_suffix('\r').unwrap_or(line);
    if line.contains(['\r', '\n']) {
        return Err(AcpError { code: -32700, message: "embedded newline".into() });
    }
    let v: Value = serde_json::from_str(line)
        .map_err(|e| AcpError { code: -32700, message: e.to_string() })?;
    let obj = v.as_object().ok_or(AcpError { code: -32600, message: "not an object".into() })?;
    let id = obj.get("id");
    let method = obj.get("method").and_then(Value::as_str);
    match (id, method) {
        (Some(id), Some(m)) => Ok(AcpMessage::Request {
            id: id.clone(),
            method: m.to_string(),
            params: obj.get("params").cloned().unwrap_or(Value::Null),
        }),
        (None, Some(m)) => Ok(AcpMessage::Notification {
            method: m.to_string(),
            params: obj.get("params").cloned().unwrap_or(Value::Null),
        }),
        (Some(id), None) => {
            let error = obj.get("error").map(|e| AcpError {
                code: e.get("code").and_then(Value::as_i64).unwrap_or(-32603),
                message: e
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            });
            Ok(AcpMessage::Response {
                id: id.clone(),
                result: obj.get("result").cloned(),
                error,
            })
        }
        (None, None) => {
            Err(AcpError { code: -32600, message: "no id or method".into() })
        }
    }
}

// ---------------------------------------------------- client -> agent

/// `initialize` with the capabilities this client advertises. `fs` and
/// `terminal` must stay false until the review/permission UI owns them.
pub fn initialize(id: i64, name: &str, version: &str, fs: bool, terminal: bool) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": { "readTextFile": fs, "writeTextFile": fs },
                "terminal": terminal,
            },
            "clientInfo": { "name": name, "version": version },
        }
    })
}

/// `authenticate` with a method id from the agent's `authMethods`.
pub fn authenticate(id: i64, method_id: &str) -> Value {
    json!({
        "jsonrpc": "2.0", "id": id, "method": "authenticate",
        "params": { "methodId": method_id },
    })
}

/// `session/new` rooted at an absolute working directory.
pub fn session_new(id: i64, cwd: &str) -> Value {
    json!({
        "jsonrpc": "2.0", "id": id, "method": "session/new",
        "params": { "cwd": cwd, "mcpServers": [] },
    })
}

/// `session/load` to resume a previous session. Callers MUST have
/// checked the agent's `loadSession` capability first (spec: agents
/// without it MUST NOT get this call). The agent replays the
/// conversation as session/update notifications.
pub fn session_load(id: i64, session_id: &str, cwd: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "session/load",
        "params": { "sessionId": session_id, "cwd": cwd, "mcpServers": [] },
    })
}

/// `session/prompt` with plain-text content blocks.
pub fn session_prompt(id: i64, session_id: &str, text: &str) -> Value {
    json!({
        "jsonrpc": "2.0", "id": id, "method": "session/prompt",
        "params": {
            "sessionId": session_id,
            "prompt": [{ "type": "text", "text": text }],
        },
    })
}

/// `session/cancel` notification (no response expected).
pub fn session_cancel(session_id: &str) -> Value {
    json!({
        "jsonrpc": "2.0", "method": "session/cancel",
        "params": { "sessionId": session_id },
    })
}

// ---------------------------------------------------- agent -> client

/// What the agent negotiated in its `initialize` result.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentHello {
    pub protocol_version: i64,
    pub name: String,
    pub version: String,
    pub auth_methods: Vec<String>,
    pub load_session: bool,
}

pub fn parse_hello(result: &Value) -> Result<AgentHello, AcpError> {
    let bad = || AcpError { code: -32602, message: "bad initialize result".into() };
    let protocol_version = result
        .get("protocolVersion")
        .and_then(Value::as_i64)
        .ok_or_else(bad)?;
    let info = result.get("agentInfo").cloned().unwrap_or(json!({}));
    let auth_methods = result
        .get("authMethods")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|m| m.get("id").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let load_session = result
        .get("agentCapabilities")
        .and_then(|c| c.get("loadSession"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(AgentHello {
        protocol_version,
        name: info.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
        version: info.get("version").and_then(Value::as_str).unwrap_or("").to_string(),
        auth_methods,
        load_session,
    })
}

/// Session id from a `session/new` result.
pub fn parse_session_id(result: &Value) -> Option<String> {
    result.get("sessionId").and_then(Value::as_str).map(str::to_string)
}

/// Stop reason from a `session/prompt` result.
pub fn parse_stop_reason(result: &Value) -> Option<String> {
    result.get("stopReason").and_then(Value::as_str).map(str::to_string)
}

/// The subset of `session/update` payloads the UI needs first.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionUpdate {
    AgentChunk { text: String },
    Plan { entries: Vec<String> },
    ToolCall { id: String, title: String, status: String },
    Usage { used: i64, size: i64 },
    Other { kind: String },
}

pub fn parse_session_update(params: &Value) -> Option<(String, SessionUpdate)> {
    let session_id = params.get("sessionId")?.as_str()?.to_string();
    let u = params.get("update")?;
    let kind = u.get("sessionUpdate")?.as_str()?;
    let update = match kind {
        // user_message_chunk appears in session/load replays.
        "agent_message_chunk" | "agent_thought_chunk" | "user_message_chunk" => {
            SessionUpdate::AgentChunk {
                text: u
                    .get("content")
                    .and_then(|c| c.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            }
        }
        "plan" => SessionUpdate::Plan {
            entries: u
                .get("entries")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|e| {
                            let c = e.get("content").and_then(Value::as_str)?;
                            let s = e.get("status").and_then(Value::as_str).unwrap_or("pending");
                            Some(format!("{c} ({s})"))
                        })
                        .collect()
                })
                .unwrap_or_default(),
        },
        "tool_call" | "tool_call_update" => SessionUpdate::ToolCall {
            id: u.get("toolCallId").and_then(Value::as_str).unwrap_or("").to_string(),
            title: u.get("title").and_then(Value::as_str).unwrap_or("").to_string(),
            status: u.get("status").and_then(Value::as_str).unwrap_or("").to_string(),
        },
        "usage_update" => SessionUpdate::Usage {
            used: u.get("used").and_then(Value::as_i64).unwrap_or(0),
            size: u.get("size").and_then(Value::as_i64).unwrap_or(0),
        },
        other => SessionUpdate::Other { kind: other.to_string() },
    };
    Some((session_id, update))
}

/// An agent's `fs/read_text_file` request, to be answered by the
/// (separate, permission-gated) filesystem integration.
#[derive(Clone, Debug, PartialEq)]
pub struct FsRead {
    pub session_id: String,
    pub path: String,
    pub line: Option<u64>,
    pub limit: Option<u64>,
}

pub fn parse_fs_read(params: &Value) -> Option<FsRead> {
    Some(FsRead {
        session_id: params.get("sessionId")?.as_str()?.to_string(),
        path: params.get("path")?.as_str()?.to_string(),
        line: params.get("line").and_then(Value::as_u64),
        limit: params.get("limit").and_then(Value::as_u64),
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct FsWrite {
    pub session_id: String,
    pub path: String,
    pub content: String,
}

pub fn parse_fs_write(params: &Value) -> Option<FsWrite> {
    Some(FsWrite {
        session_id: params.get("sessionId")?.as_str()?.to_string(),
        path: params.get("path")?.as_str()?.to_string(),
        content: params.get("content")?.as_str()?.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_roundtrip_and_newline_rules() {
        let msg = session_prompt(7, "s1", "hello\nworld");
        let wire = encode(&msg).unwrap();
        assert!(wire.ends_with('\n') && !wire[..wire.len() - 1].contains('\n'));
        match decode(&wire).unwrap() {
            AcpMessage::Request { id, method, params } => {
                assert_eq!(id, json!(7));
                assert_eq!(method, "session/prompt");
                assert_eq!(params["sessionId"], json!("s1"));
                assert_eq!(params["prompt"][0]["text"], json!("hello\nworld"));
            }
            _ => panic!("not a request"),
        }
        assert!(decode("{\"jsonrpc\":\"2.0\",\"id\":1}\nrest").is_err());
        assert!(decode("not json").is_err());
        assert!(decode("{\"jsonrpc\":\"2.0\"}").is_err());
    }

    #[test]
    fn client_requests_match_the_spec_examples() {
        let init = initialize(0, "editor.izdrail.com", "0.1", false, false);
        assert_eq!(init["params"]["protocolVersion"], json!(1));
        assert_eq!(init["params"]["clientCapabilities"]["fs"]["readTextFile"], json!(false));
        assert_eq!(init["params"]["clientCapabilities"]["terminal"], json!(false));
        assert_eq!(init["params"]["clientInfo"]["name"], json!("editor.izdrail.com"));

        let new = session_new(1, "/home/stefan/project");
        assert_eq!(new["method"], json!("session/new"));
        assert_eq!(new["params"]["cwd"], json!("/home/stefan/project"));
        assert_eq!(new["params"]["mcpServers"], json!([]));

        let cancel = session_cancel("s1");
        assert_eq!(cancel.get("id"), None);
        match decode(&encode(&cancel).unwrap()).unwrap() {
            AcpMessage::Notification { method, .. } => assert_eq!(method, "session/cancel"),
            _ => panic!("cancel must be a notification"),
        }
        let auth = authenticate(2, "claude-login");
        assert_eq!(auth["params"]["methodId"], json!("claude-login"));
    }

    #[test]
    fn agent_messages_are_parsed() {
        let hello = parse_hello(&json!({
            "protocolVersion": 1,
            "agentCapabilities": { "loadSession": true },
            "agentInfo": { "name": "opencode", "version": "0.6.3" },
            "authMethods": [{ "id": "oauth" }],
        }))
        .unwrap();
        let expected_hello = AgentHello {
            protocol_version: 1,
            name: "opencode".into(),
            version: "0.6.3".into(),
            auth_methods: vec!["oauth".into()],
            load_session: true,
        };
        assert_eq!(hello, expected_hello);
        assert!(parse_hello(&json!({})).is_err());

        assert_eq!(parse_session_id(&json!({"sessionId": "sess_1"})), Some("sess_1".into()));
        assert_eq!(parse_stop_reason(&json!({"stopReason": "end_turn"})), Some("end_turn".into()));

        let (sid, update) = parse_session_update(&json!({
            "sessionId": "sess_1",
            "update": { "sessionUpdate": "agent_message_chunk",
                        "content": { "type": "text", "text": "working" } },
        }))
        .unwrap();
        assert_eq!(sid, "sess_1");
        let expected_chunk = SessionUpdate::AgentChunk { text: "working".into() };
        assert_eq!(update, expected_chunk);

        let (_, plan) = parse_session_update(&json!({
            "sessionId": "s",
            "update": { "sessionUpdate": "plan", "entries": [
                { "content": "Read code", "status": "pending" },
                { "content": "Edit", "status": "in_progress" } ] },
        }))
        .unwrap();
        let expected_plan = SessionUpdate::Plan {
            entries: vec!["Read code (pending)".into(), "Edit (in_progress)".into()],
        };
        assert_eq!(plan, expected_plan);

        let (_, tc) = parse_session_update(&json!({
            "sessionId": "s",
            "update": { "sessionUpdate": "tool_call", "toolCallId": "c1",
                        "title": "Edit file", "status": "pending" },
        }))
        .unwrap();
        let expected_tc = SessionUpdate::ToolCall {
            id: "c1".into(),
            title: "Edit file".into(),
            status: "pending".into(),
        };
        assert_eq!(tc, expected_tc);

        let read = parse_fs_read(&json!({
            "sessionId": "s", "path": "/p/a.rs", "line": 10, "limit": 50,
        }))
        .unwrap();
        let expected_read = FsRead {
            session_id: "s".into(),
            path: "/p/a.rs".into(),
            line: Some(10),
            limit: Some(50),
        };
        assert_eq!(read, expected_read);
        let write = parse_fs_write(&json!({
            "sessionId": "s", "path": "/p/a.rs", "content": "x",
        }))
        .unwrap();
        assert_eq!(write.content, "x");
        assert!(parse_fs_read(&json!({"path": "/p"})).is_none());
        assert!(parse_fs_write(&json!({"sessionId": "s", "path": "/p"})).is_none());
    }
}
