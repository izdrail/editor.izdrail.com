//! MCP server session: the lifecycle state machine for one stdio MCP
//! server - initialize handshake, tools/list pagination, and tools/call
//! plumbing. Pure logic like agent_acp_session: the caller owns the
//! transport (the shared NDJSON process wrapper), shuttles the emitted
//! `Value`s onto the wire, and feeds decoded `AcpMessage`s back in.
//! PERMISSION IS THE CALLER'S JOB: `begin_call` only builds the request;
//! the panel wires calls only behind the Act-tier review gate. Verified
//! against modelcontextprotocol.io/specification/2025-06-18 on
//! 2026-10-17.

use serde_json::{Value, json};

use crate::agent_acp::AcpMessage;
use crate::agent_acp_process::next_id;
use crate::mcp::{self, ToolOutcome, ToolSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum McpPhase {
    /// `initialize` sent, hello not yet accepted.
    Initializing,
    /// Hello accepted, `initialized` sent, first `tools/list` in flight.
    Listing,
    /// Tools known, idle.
    Ready,
    /// Unrecoverable failure; kill the server.
    Closed,
}

#[derive(Clone, Debug, PartialEq)]
pub enum McpEvent {
    Connected { server: String, version: String, instructions: Option<String> },
    /// The (cumulative) tool list changed; read `session.tools()`.
    ToolsListed { count: usize },
    ToolResult { name: String, text: String, is_error: bool, skipped_blocks: usize },
    /// Logs and tolerated oddities (unknown notifications).
    Notice { text: String },
    Error { message: String },
    Closed,
}

pub struct McpSession {
    phase: McpPhase,
    init_id: i64,
    list_id: Option<i64>,
    call: Option<(i64, String)>,
    tools: Vec<ToolSpec>,
}

fn error_response(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

impl McpSession {
    /// Begin the handshake; send the returned `initialize` message.
    pub fn start(client_name: &str, client_version: &str) -> (Self, Value) {
        let init_id = next_id();
        let session = McpSession {
            phase: McpPhase::Initializing,
            init_id,
            list_id: None,
            call: None,
            tools: Vec::new(),
        };
        (session, mcp::initialize(init_id, client_name, client_version))
    }

    pub fn phase(&self) -> McpPhase {
        self.phase
    }

    pub fn tools(&self) -> &[ToolSpec] {
        &self.tools
    }

    /// Build a `tools/call` request. THE CALLER OWNS PERMISSION: this
    /// must only be reachable behind the Act-tier review gate.
    pub fn begin_call(&mut self, name: &str, arguments: &Value) -> Result<Value, String> {
        if self.phase != McpPhase::Ready {
            return Err("MCP server is not ready for a call".into());
        }
        if self.call.is_some() {
            return Err("one MCP call at a time per server".into());
        }
        if !self.tools.iter().any(|t| t.name == name) {
            return Err(format!("unknown tool {name}"));
        }
        let id = next_id();
        self.call = Some((id, name.to_string()));
        Ok(mcp::tools_call(id, name, arguments))
    }

    /// Feed one decoded inbound message; returns UI events and outbound
    /// messages for the wire.
    pub fn handle(&mut self, message: &AcpMessage) -> (Vec<McpEvent>, Vec<Value>) {
        let mut events = Vec::new();
        let mut out = Vec::new();
        match message {
            AcpMessage::Response { id, result, error } => {
                let numeric = id.as_i64();
                if let Some(e) = error {
                    if numeric == Some(self.init_id) {
                        self.phase = McpPhase::Closed;
                        events.push(McpEvent::Closed);
                    }
                    if numeric == self.list_id {
                        self.list_id = None;
                        self.phase = McpPhase::Ready;
                    }
                    if let Some((call_id, name)) = self.call.clone() {
                        if numeric == Some(call_id) {
                            self.call = None;
                            events.push(McpEvent::ToolResult {
                                name,
                                text: e.message.clone(),
                                is_error: true,
                                skipped_blocks: 0,
                            });
                            return (events, out);
                        }
                    }
                    events.push(McpEvent::Error { message: e.message.clone() });
                    return (events, out);
                }
                let result = result.clone().unwrap_or_else(|| json!({}));
                if numeric == Some(self.init_id) {
                    match mcp::parse_hello(&result) {
                        Ok(hello) => {
                            events.push(McpEvent::Connected {
                                server: hello.name,
                                version: hello.version,
                                instructions: hello.instructions,
                            });
                            out.push(mcp::initialized_notification());
                            let list_id = next_id();
                            self.list_id = Some(list_id);
                            self.phase = McpPhase::Listing;
                            out.push(mcp::tools_list(list_id, None));
                        }
                        Err(message) => {
                            events.push(McpEvent::Error { message });
                            self.phase = McpPhase::Closed;
                            events.push(McpEvent::Closed);
                        }
                    }
                } else if numeric == self.list_id {
                    let (mut page, cursor) = mcp::parse_tools_list(&result);
                    self.tools.append(&mut page);
                    match cursor {
                        Some(cursor) => {
                            let list_id = next_id();
                            self.list_id = Some(list_id);
                            out.push(mcp::tools_list(list_id, Some(&cursor)));
                        }
                        None => {
                            self.list_id = None;
                            self.phase = McpPhase::Ready;
                            events.push(McpEvent::ToolsListed { count: self.tools.len() });
                        }
                    }
                } else if let Some((call_id, name)) = self.call.clone() {
                    if numeric == Some(call_id) {
                        self.call = None;
                        let outcome: ToolOutcome = mcp::parse_tool_result(&result);
                        events.push(McpEvent::ToolResult {
                            name,
                            text: outcome.text,
                            is_error: outcome.is_error,
                            skipped_blocks: outcome.skipped_blocks,
                        });
                    } else {
                        events.push(McpEvent::Notice { text: "unmatched response id".into() });
                    }
                } else {
                    events.push(McpEvent::Notice { text: "unmatched response id".into() });
                }
            }
            AcpMessage::Notification { method, .. } => match method.as_str() {
                "notifications/tools/list_changed" => {
                    if self.phase == McpPhase::Ready {
                        let list_id = next_id();
                        self.list_id = Some(list_id);
                        self.phase = McpPhase::Listing;
                        out.push(mcp::tools_list(list_id, None));
                    }
                }
                other => {
                    events.push(McpEvent::Notice { text: format!("unhandled notification {other}") });
                }
            },
            AcpMessage::Request { id, method, .. } => {
                // We advertise no client capabilities, so every
                // server-to-client request is unsupported by definition.
                out.push(error_response(id, -32601, &format!("unsupported method {method}")));
            }
        }
        (events, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(id: i64, result: Value) -> AcpMessage {
        AcpMessage::Response { id: json!(id), result: Some(result), error: None }
    }

    fn ready_session() -> (McpSession, i64) {
        let (mut session, init) = McpSession::start("lapce", "0.1");
        let init_id = init["id"].as_i64().unwrap();
        let hello = json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "fileserv", "version": "2.0" },
        });
        let (events, out) = session.handle(&response(init_id, hello));
        assert_eq!(events.len(), 1);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["method"], json!("notifications/initialized"));
        assert_eq!(out[1]["method"], json!("tools/list"));
        let list_id = out[1]["id"].as_i64().unwrap();
        (session, list_id)
    }

    #[test]
    fn handshake_lists_tools_with_pagination() {
        let (mut session, list_id) = ready_session();
        let page1 = json!({
            "tools": [ { "name": "read", "description": "läs 🦀" } ],
            "nextCursor": "p2",
        });
        let (events, out) = session.handle(&response(list_id, page1));
        assert!(events.is_empty());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["params"]["cursor"], json!("p2"));
        let page2_id = out[0]["id"].as_i64().unwrap();
        let page2 = json!({ "tools": [ { "name": "write", "description": "skriv" } ] });
        let (events, out) = session.handle(&response(page2_id, page2));
        assert!(out.is_empty());
        assert_eq!(session.phase(), McpPhase::Ready);
        let expected_listed = McpEvent::ToolsListed { count: 2 };
        assert_eq!(events[0], expected_listed);
        assert_eq!(session.tools()[0].description, "läs 🦀");
    }

    #[test]
    fn calls_round_trip_and_gate() {
        let (mut session, list_id) = ready_session();
        let page = json!({ "tools": [ { "name": "read", "description": "r" } ] });
        let _ = session.handle(&response(list_id, page));
        let call = session.begin_call("read", &json!({ "path": "a.php" })).unwrap();
        assert_eq!(call["method"], json!("tools/call"));
        assert!(session.begin_call("read", &json!({})).is_err());
        assert!(session.begin_call("missing", &json!({})).is_err());
        let call_id = call["id"].as_i64().unwrap();
        let result = json!({ "content": [ { "type": "text", "text": "done" } ] });
        let (events, _) = session.handle(&response(call_id, result));
        let expected_result = McpEvent::ToolResult {
            name: "read".into(),
            text: "done".into(),
            is_error: false,
            skipped_blocks: 0,
        };
        assert_eq!(events[0], expected_result);
        let error = crate::agent_acp::AcpError { code: -32603, message: "boom".into() };
        let failure = AcpMessage::Response { id: json!(99), result: None, error: Some(error) };
        let (events, _) = session.handle(&failure);
        let expected_error = McpEvent::Error { message: "boom".into() };
        assert_eq!(events.last().unwrap(), &expected_error);
    }

    #[test]
    fn list_changed_triggers_relist() {
        let (mut session, list_id) = ready_session();
        let page = json!({ "tools": [] });
        let _ = session.handle(&response(list_id, page));
        let note = AcpMessage::Notification {
            method: "notifications/tools/list_changed".into(),
            params: json!({}),
        };
        let (_, out) = session.handle(&note);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["method"], json!("tools/list"));
        assert_eq!(session.phase(), McpPhase::Listing);
    }

    #[test]
    fn server_requests_are_refused_and_init_errors_close() {
        let (mut session, _) = ready_session();
        let request = AcpMessage::Request {
            id: json!(55),
            method: "sampling/createMessage".into(),
            params: json!({}),
        };
        let (_, out) = session.handle(&request);
        assert_eq!(out[0]["error"]["code"], json!(-32601));

        let (mut fresh, init) = McpSession::start("lapce", "0.1");
        let init_id = init["id"].as_i64().unwrap();
        let error = crate::agent_acp::AcpError { code: -32602, message: "too old".into() };
        let failure = AcpMessage::Response { id: json!(init_id), result: None, error: Some(error) };
        let (events, _) = fresh.handle(&failure);
        assert_eq!(fresh.phase(), McpPhase::Closed);
        let expected_closed = McpEvent::Closed;
        assert_eq!(events[0], expected_closed);
        drop(session);
    }
}
