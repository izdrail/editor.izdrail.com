//! Workspace MCP tools in the agent loop (Act mode). At run start the
//! panel connects each enabled `.lapce/mcp.json` server on the worker
//! thread, keeps the sessions alive for the run, and exposes their
//! tools to the model as `mcp__server__tool` functions. Every call
//! goes through the chat panel's approval lane (the owner sees the
//! tool and its arguments and approves or denies) - this module never
//! asks and never decides, it only connects, lists and calls. Tool
//! results are untrusted data. Linux-only like the rest of the
//! process lanes; sessions die with the pool (AcpProcess kills its
//! child on drop).

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

use crate::agent_acp_process::AcpProcess;
use crate::mcp::{self, McpServer, ToolSpec};
use crate::mcp_session::{McpEvent, McpPhase, McpSession};

const MAX_SERVERS: usize = 4;
const MAX_TOOLS: usize = 16;
const HANDSHAKE_MESSAGE_CAP: usize = 64;
const CALL_MESSAGE_CAP: usize = 64;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

struct LiveServer {
    name: String,
    process: AcpProcess,
    session: McpSession,
    tools: Vec<ToolSpec>,
}

pub struct McpPool {
    servers: Vec<LiveServer>,
}

impl McpPool {
    pub fn empty() -> Self {
        Self { servers: Vec::new() }
    }

    /// Connect the workspace's enabled servers. A server that fails to
    /// launch or handshake becomes a note, never a run failure: the
    /// chat lane must survive a stale mcp.json.
    pub fn connect(root: &Path) -> (Self, Vec<String>) {
        let Ok(text) = std::fs::read_to_string(root.join(".lapce").join("mcp.json"))
        else {
            return (Self::empty(), Vec::new());
        };
        let servers: Vec<McpServer> = match mcp::parse_config(&text) {
            Ok(config) => config
                .servers
                .into_iter()
                .filter(|s| s.enabled)
                .take(MAX_SERVERS)
                .collect(),
            Err(e) => return (Self::empty(), vec![format!("MCP config error: {e}")]),
        };
        let mut pool = Self::empty();
        let mut notes = Vec::new();
        for server in servers {
            match connect_server(&server, root) {
                Ok(live) => {
                    notes.push(format!(
                        "MCP server {} connected ({} tools)",
                        live.name,
                        live.tools.len()
                    ));
                    pool.servers.push(live);
                }
                Err(e) => notes.push(format!("MCP server {} skipped: {e}", server.name)),
            }
        }
        (pool, notes)
    }

    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }

    /// Function schemas for the loop, one per tool, names sanitized to
    /// [A-Za-z0-9_-] and capped, total tool count capped.
    pub fn schemas(&self) -> Vec<Value> {
        let mut out = Vec::new();
        for server in &self.servers {
            for tool in &server.tools {
                if out.len() == MAX_TOOLS {
                    return out;
                }
                let name = qualified_name(&server.name, &tool.name);
                if name.len() > 128 || !tool.input_schema.is_object() {
                    continue;
                }
                let description = if tool.description.is_empty() {
                    format!("Workspace MCP tool from server {}", server.name)
                } else {
                    tool.description.chars().take(512).collect()
                };
                out.push(serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": name,
                        "description": description,
                        "parameters": tool.input_schema,
                    }
                }));
            }
        }
        out
    }

    /// Short approval text for the panel: which server/tool and a
    /// bounded preview of the arguments.
    pub fn summary(tool: &str, arguments: &Value) -> String {
        let args = arguments.to_string();
        let args: String = args.chars().take(300).collect();
        format!("mcp {tool} {args}")
    }

    pub fn call(&mut self, qualified: &str, arguments: &Value) -> Result<String, String> {
        for live in &mut self.servers {
            let prefix = format!("mcp__{}__", sanitize(&live.name));
            let Some(tool_name) = qualified.strip_prefix(&prefix) else {
                continue;
            };
            if !live
                .tools
                .iter()
                .any(|t| sanitize(&t.name) == tool_name)
            {
                return Err(format!("MCP server {} has no tool {tool_name}", live.name));
            }
            let original = live
                .tools
                .iter()
                .find(|t| sanitize(&t.name) == tool_name)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            let request = live.session.begin_call(&original, arguments)?;
            live.process.send(&request).map_err(|e| e.message)?;
            let mut guard = 0;
            loop {
                guard += 1;
                if guard > CALL_MESSAGE_CAP {
                    return Err("MCP call did not finish after 64 messages".into());
                }
                let Some(message) = live.process.recv().map_err(|e| e.message)? else {
                    return Err("MCP server exited mid-call".into());
                };
                let (events, out) = live.session.handle(&message);
                for value in out {
                    live.process.send(&value).map_err(|e| e.message)?;
                }
                for event in events {
                    match event {
                        McpEvent::ToolResult { text, is_error, .. } => {
                            return if is_error {
                                Err(format!("MCP tool reported an error: {text}"))
                            } else {
                                Ok(text)
                            };
                        }
                        McpEvent::Error { message } => return Err(message),
                        McpEvent::Closed => {
                            return Err("MCP server closed the session".into())
                        }
                        _ => {}
                    }
                }
            }
        }
        Err(format!("Unregistered MCP tool: {qualified}"))
    }
}

/// `mcp__server__tool`, with every segment sanitized to the loop's
/// accepted name alphabet.
fn qualified_name(server: &str, tool: &str) -> String {
    format!("mcp__{}__{}", sanitize(server), sanitize(tool))
}

fn sanitize(segment: &str) -> String {
    segment
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

/// Spawn and handshake one server with a hard timeout: the blocking
/// reads run on a collector thread, and a server that does not finish
/// the handshake in time is killed by pid so a hung server cannot
/// wedge the agent worker thread.
fn connect_server(server: &McpServer, root: &Path) -> Result<LiveServer, String> {
    let args: Vec<String> = server.args.clone();
    let root = root.to_path_buf();
    let command = server.command.clone();
    let pid_slot: Arc<Mutex<Option<u32>>> = Arc::new(Mutex::new(None));
    let name = server.name.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let slot = pid_slot.clone();
    std::thread::spawn(move || {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let result = connect_blocking(&name, &command, &args, &root, &slot);
        let _ = tx.send(result);
    });
    match rx.recv_timeout(CONNECT_TIMEOUT) {
        Ok(result) => result,
        Err(_) => {
            if let Some(pid) = pid_slot.lock().unwrap().take() {
                let _ = std::process::Command::new("kill")
                    .arg(pid.to_string())
                    .status();
            }
            Err("timed out during handshake".into())
        }
    }
}

fn connect_blocking(
    name: &str,
    command: &str,
    args: &[&str],
    root: &Path,
    pid_slot: &Arc<Mutex<Option<u32>>>,
) -> Result<LiveServer, String> {
    let mut process = AcpProcess::spawn(command, args, root).map_err(|e| e.to_string())?;
    *pid_slot.lock().unwrap() = Some(process.id());
    let (mut session, init) = McpSession::start("gravity", env!("CARGO_PKG_VERSION"));
    process.send(&init).map_err(|e| e.message)?;
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > HANDSHAKE_MESSAGE_CAP {
            return Err("handshake did not finish after 64 messages".into());
        }
        let Some(message) = process.recv().map_err(|e| e.message)? else {
            return Err("server exited during setup".into());
        };
        let (events, out) = session.handle(&message);
        for value in out {
            process.send(&value).map_err(|e| e.message)?;
        }
        for event in &events {
            match event {
                McpEvent::Error { message } => return Err(message.clone()),
                McpEvent::Closed => return Err("server closed the session".into()),
                _ => {}
            }
        }
        if session.phase() == McpPhase::Ready {
            let tools = session.tools().to_vec();
            return Ok(LiveServer {
                name: name.to_string(),
                process,
                session,
                tools,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec(name: &str, description: &str) -> ToolSpec {
        ToolSpec {
            name: name.into(),
            title: None,
            description: description.into(),
            input_schema: json!({"type":"object","properties":{}}),
        }
    }

    #[test]
    fn schema_names_are_sanitized_and_qualified() {
        let mut pool = McpPool::empty();
        pool.servers.push(LiveServer {
            name: "fs server".into(),
            process: spawn_cat(),
            session: McpSession::start("gravity", "0").0,
            tools: vec![spec("read file", "Reads a file"), spec("write!", "")],
        });
        let schemas = pool.schemas();
        assert_eq!(schemas.len(), 2);
        assert_eq!(schemas[0]["function"]["name"], "mcp__fs_server__read_file");
        assert_eq!(schemas[0]["function"]["description"], "Reads a file");
        assert_eq!(schemas[1]["function"]["name"], "mcp__fs_server__write_");
        // Fallback description when the server gives none.
        assert!(schemas[1]["function"]["description"]
            .as_str()
            .unwrap()
            .contains("fs server"));
        assert!(schemas[0]["function"]["parameters"].is_object());
    }

    #[test]
    fn summary_is_bounded() {
        let args = json!({"text":"x".repeat(1000)});
        let summary = McpPool::summary("mcp__fs__read", &args);
        assert!(summary.starts_with("mcp mcp__fs__read {"));
        assert!(summary.len() < 320);
    }

    #[test]
    fn connect_missing_config_is_empty_and_dead_binary_is_a_note() {
        let dir = tempfile::tempdir().unwrap();
        let (pool, notes) = McpPool::connect(dir.path());
        assert!(pool.is_empty());
        assert!(notes.is_empty());

        std::fs::create_dir(dir.path().join(".lapce")).unwrap();
        std::fs::write(
            dir.path().join(".lapce").join("mcp.json"),
            r#"{"servers":[{"name":"ghost","command":"/nonexistent/mcp-server","args":[],"enabled":true},{"name":"off","command":"/also/missing","args":[],"enabled":false}]}"#,
        )
        .unwrap();
        let (pool, notes) = McpPool::connect(dir.path());
        assert!(pool.is_empty());
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("ghost skipped"), "note: {}", notes[0]);
    }

    #[test]
    fn call_roundtrips_through_a_fake_sh_server() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".lapce")).unwrap();
        let script = r#"{"servers":[{"name":"fake","command":"sh","args":["-c","while IFS= read -r line; do id=$(printf '%s' \"$line\" | sed -n 's/.*\"id\":\\([0-9]*\\).*/\\1/p'); case \"$line\" in *'notifications/initialized'*) : ;; *'initialize'*) printf '%s\n' '{\"jsonrpc\":\"2.0\",\"id\":'$id',\"result\":{\"protocolVersion\":\"2025-03-26\",\"capabilities\":{\"tools\":{}},\"serverInfo\":{\"name\":\"fake\",\"version\":\"0\"}}}' ;; *'tools/list'*) printf '%s\n' '{\"jsonrpc\":\"2.0\",\"id\":'$id',\"result\":{\"tools\":[{\"name\":\"echo\",\"description\":\"Echo back\",\"inputSchema\":{\"type\":\"object\",\"properties\":{\"text\":{\"type\":\"string\"}}}}]}}' ;; *'tools/call'*) printf '%s\n' '{\"jsonrpc\":\"2.0\",\"id\":'$id',\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"echo-ok\"}]}}' ;; esac; done"],"enabled":true}]}"#;
        std::fs::write(dir.path().join(".lapce").join("mcp.json"), script).unwrap();
        let (mut pool, notes) = McpPool::connect(dir.path());
        assert!(notes.iter().any(|n| n.contains("fake connected")), "notes: {notes:?}");
        let schemas = pool.schemas();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0]["function"]["name"], "mcp__fake__echo");
        let out = pool
            .call("mcp__fake__echo", &json!({"text":"hello"}))
            .unwrap();
        assert_eq!(out, "echo-ok");
        // Unknown server and unknown tool are named errors.
        assert!(pool
            .call("mcp__other__echo", &json!({}))
            .unwrap_err()
            .contains("Unregistered MCP tool"));
        assert!(pool
            .call("mcp__fake__nope", &json!({}))
            .unwrap_err()
            .contains("no tool"));
    }

    /// The schema test needs a LiveServer but not a live protocol: a
    /// `cat` subprocess satisfies the process slot and is killed on
    /// drop like any session.
    fn spawn_cat() -> AcpProcess {
        AcpProcess::spawn("cat", &[], std::path::Path::new("/tmp")).unwrap()
    }
}
