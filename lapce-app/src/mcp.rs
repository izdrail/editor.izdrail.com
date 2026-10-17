//! MCP (Model Context Protocol) client contract: the message shapes for
//! talking to stdio MCP servers (tools/list, tools/call) plus the
//! workspace-scoped `.lapce/mcp.json` server configuration. Pure shapes
//! and parsing - no process spawn, no UI, no tool execution. Wire
//! framing (NDJSON) reuses agent_acp::encode/decode at the transport
//! layer. Verified against modelcontextprotocol.io/specification/
//! 2025-06-18 (basic/lifecycle, server/tools) on 2026-10-17.
//!
//! Trust: MCP tools are arbitrary server-side code, so tools/call is
//! gated at the same tier as Act (human-in-the-loop per the spec's own
//! guidance). This module only describes messages; the session layer
//! owns the lifecycle and the panel owns permission.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The protocol revision this client speaks (date-versioned by MCP).
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// Client `initialize`. We advertise NO client capabilities (no roots,
/// sampling or elicitation) until those surfaces exist.
pub fn initialize(id: i64, name: &str, version: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": name, "version": version },
        }
    })
}

/// The `notifications/initialized` one-way message that ends the
/// handshake. The spec requires it before normal operation.
pub fn initialized_notification() -> Value {
    json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
}

/// `tools/list`, optionally continuing from a cursor.
pub fn tools_list(id: i64, cursor: Option<&str>) -> Value {
    let params = match cursor {
        Some(c) => json!({ "cursor": c }),
        None => json!({}),
    };
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/list", "params": params })
}

/// `tools/call` with a JSON arguments object.
pub fn tools_call(id: i64, name: &str, arguments: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments },
    })
}

/// What the server negotiated in its `initialize` result.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerHello {
    pub protocol_version: String,
    pub name: String,
    pub version: String,
    pub instructions: Option<String>,
    /// The server declares a tools capability.
    pub has_tools: bool,
}

pub fn parse_hello(result: &Value) -> Result<ServerHello, String> {
    let protocol_version = result
        .get("protocolVersion")
        .and_then(Value::as_str)
        .ok_or("bad initialize result: no protocolVersion")?
        .to_string();
    let info = result.get("serverInfo").cloned().unwrap_or(json!({}));
    let has_tools = result
        .get("capabilities")
        .and_then(|c| c.get("tools"))
        .is_some();
    Ok(ServerHello {
        protocol_version,
        name: info.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
        version: info.get("version").and_then(Value::as_str).unwrap_or("").to_string(),
        instructions: result
            .get("instructions")
            .and_then(Value::as_str)
            .map(str::to_string),
        has_tools,
    })
}

/// One advertised tool.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub title: Option<String>,
    pub description: String,
    pub input_schema: Value,
}

/// Parse a `tools/list` result into specs plus the next cursor.
/// Tools without a usable name are skipped, not fatal.
pub fn parse_tools_list(result: &Value) -> (Vec<ToolSpec>, Option<String>) {
    let tools = result
        .get("tools")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|t| {
                    let name = t.get("name").and_then(Value::as_str)?.to_string();
                    Some(ToolSpec {
                        name,
                        title: t
                            .get("title")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        description: t
                            .get("description")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        input_schema: t.get("inputSchema").cloned().unwrap_or(json!({})),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let cursor = result
        .get("nextCursor")
        .and_then(Value::as_str)
        .map(str::to_string);
    (tools, cursor)
}

/// A `tools/call` result: concatenated text content plus the error flag.
/// Non-text content blocks (images, resources) are counted but not read.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolOutcome {
    pub text: String,
    pub is_error: bool,
    pub skipped_blocks: usize,
}

pub fn parse_tool_result(result: &Value) -> ToolOutcome {
    let mut text = String::new();
    let mut skipped_blocks = 0;
    if let Some(blocks) = result.get("content").and_then(Value::as_array) {
        for block in blocks {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(t) = block.get("text").and_then(Value::as_str) {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(t);
                    }
                }
                _ => skipped_blocks += 1,
            }
        }
    }
    ToolOutcome {
        text,
        is_error: result.get("isError").and_then(Value::as_bool).unwrap_or(false),
        skipped_blocks,
    }
}

/// Workspace-scoped server configuration: `.lapce/mcp.json`. This file
/// can arrive via git clone, so discovery NEVER auto-spawns - the panel
/// lists configured servers and starting one is an explicit user action.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct McpConfig {
    pub servers: Vec<McpServer>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct McpServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
}

impl Default for McpServer {
    fn default() -> Self {
        Self { name: String::new(), command: String::new(), args: Vec::new(), enabled: true }
    }
}

pub fn parse_config(text: &str) -> Result<McpConfig, String> {
    let config: McpConfig =
        serde_json::from_str(text).map_err(|e| format!("bad .lapce/mcp.json: {e}"))?;
    for server in &config.servers {
        if server.name.trim().is_empty() || server.command.trim().is_empty() {
            return Err("every MCP server needs a name and a command".into());
        }
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_shapes_match_the_spec_examples() {
        let init = initialize(1, "lapce", "0.1");
        assert_eq!(init["method"], json!("initialize"));
        assert_eq!(init["params"]["protocolVersion"], json!(PROTOCOL_VERSION));
        assert_eq!(init["params"]["clientInfo"]["name"], json!("lapce"));
        let note = initialized_notification();
        assert_eq!(note["method"], json!("notifications/initialized"));
        assert!(note.get("id").is_none());
    }

    #[test]
    fn hello_parses_spec_response() {
        let result = json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": { "listChanged": true }, "logging": {} },
            "serverInfo": { "name": "ExampleServer", "version": "1.0.0" },
            "instructions": "Be careful.",
        });
        let hello = parse_hello(&result).unwrap();
        assert_eq!(hello.protocol_version, "2025-06-18");
        assert_eq!(hello.name, "ExampleServer");
        assert!(hello.has_tools);
        assert_eq!(hello.instructions.as_deref(), Some("Be careful."));
        let bare = parse_hello(&json!({ "protocolVersion": "2024-11-05" })).unwrap();
        assert!(!bare.has_tools);
        assert!(bare.instructions.is_none());
        assert!(parse_hello(&json!({})).is_err());
    }

    #[test]
    fn tools_list_parses_and_paginates() {
        let result = json!({
            "tools": [
                {
                    "name": "get_weather",
                    "title": "Weather Information Provider",
                    "description": "Get current weather information for a location",
                    "inputSchema": { "type": "object", "properties": { "location": { "type": "string" } } }
                },
                { "description": "nameless, skipped" }
            ],
            "nextCursor": "page-2",
        });
        let (tools, cursor) = parse_tools_list(&result);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "get_weather");
        assert_eq!(tools[0].title.as_deref(), Some("Weather Information Provider"));
        assert_eq!(cursor.as_deref(), Some("page-2"));
        let (tools, cursor) = parse_tools_list(&json!({ "tools": [] }));
        assert!(tools.is_empty());
        assert!(cursor.is_none());
        let request = tools_list(7, Some("page-2"));
        assert_eq!(request["params"]["cursor"], json!("page-2"));
        let first = tools_list(6, None);
        assert!(first["params"].as_object().unwrap().is_empty());
    }

    #[test]
    fn tool_result_concatenates_text_and_flags_errors() {
        let result = json!({
            "content": [
                { "type": "text", "text": "först 🦀" },
                { "type": "image", "data": "..." },
                { "type": "text", "text": "second" }
            ],
            "isError": false,
        });
        let outcome = parse_tool_result(&result);
        assert_eq!(outcome.text, "först 🦀\nsecond");
        assert_eq!(outcome.skipped_blocks, 1);
        assert!(!outcome.is_error);
        let failed = parse_tool_result(&json!({ "content": [], "isError": true }));
        assert!(failed.is_error);
        let call = tools_call(9, "get_weather", &json!({ "location": "London" }));
        assert_eq!(call["method"], json!("tools/call"));
        assert_eq!(call["params"]["name"], json!("get_weather"));
    }

    #[test]
    fn config_parses_and_validates() {
        let config = parse_config(
            r#"{ "servers": [
                { "name": "filesystem", "command": "npx", "args": ["-y", "@mcp/fs"] },
                { "name": "off", "command": "mcp-off", "enabled": false }
            ] }"#,
        )
        .unwrap();
        assert_eq!(config.servers.len(), 2);
        assert!(config.servers[0].enabled);
        assert!(!config.servers[1].enabled);
        assert_eq!(config.servers[0].args[1], "@mcp/fs");
        let empty = parse_config("{}").unwrap();
        assert!(empty.servers.is_empty());
        assert!(parse_config("{ not json").is_err());
        let nameless = parse_config(r#"{ "servers": [ { "command": "x" } ] }"#);
        assert!(nameless.is_err());
        let non_ascii = parse_config(
            r#"{ "servers": [ { "name": "tööl", "command": "tööl", "args": ["--därg"] } ] }"#,
        )
        .unwrap();
        assert_eq!(non_ascii.servers[0].args[0], "--därg");
    }
}
