//! MCP connect-and-report: spawn each enabled workspace server,
//! handshake, list its tools, and render a Markdown status buffer the
//! command palette action opens. Runs OFF the UI thread (blocking
//! reads). Each server gets a hard timeout on a collector thread; a
//! timed-out server is killed by pid (Linux) so a hung server cannot
//! wedge the probe. Connections are probe-and-disconnect: long-lived
//! servers belong to the agent-loop integration item, and tool calls
//! stay disabled until the Act review UI owns them.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::agent_acp_process::AcpProcess;
use crate::mcp::{self, McpServer};
use crate::mcp_session::{McpEvent, McpPhase, McpSession};

const HANDSHAKE_MESSAGE_CAP: usize = 64;

pub struct ServerReport {
    pub name: String,
    pub command: String,
    pub status: String,
    pub tools: Vec<(String, String)>,
}

/// Build the report markdown and choose where it should be written.
pub fn run_report(workspace_local: bool, root: Option<PathBuf>) -> (String, PathBuf) {
    let root = match root {
        Some(r) if workspace_local => r.canonicalize().unwrap_or(r),
        _ => {
            let markdown = render_report(&[], false, false);
            return (markdown, std::env::temp_dir().join("lapce-mcp-status.md"));
        }
    };
    let status_path = root.join(".lapce").join("mcp").join("status.md");
    let config_text = std::fs::read_to_string(root.join(".lapce").join("mcp.json")).ok();
    let (missing, servers) = match &config_text {
        None => (true, Vec::new()),
        Some(text) => match mcp::parse_config(text) {
            Ok(config) => (
                false,
                config.servers.into_iter().filter(|s| s.enabled).collect::<Vec<_>>(),
            ),
            Err(e) => {
                let report = ServerReport {
                    name: "config".into(),
                    command: String::new(),
                    status: format!("error: {e}"),
                    tools: Vec::new(),
                };
                return (render_report(&[report], false, true), status_path);
            }
        },
    };
    let reports: Vec<ServerReport> = servers
        .iter()
        .map(|s| probe_server(s, &root, Duration::from_secs(10)))
        .collect();
    (render_report(&reports, missing, true), status_path)
}

/// Pure markdown rendering, unit-tested.
pub fn render_report(reports: &[ServerReport], missing_config: bool, workspace_open: bool) -> String {
    let mut out = String::from("# MCP servers\n\n");
    if !workspace_open {
        out.push_str(
            "Open a local project folder first; servers are configured per workspace in .lapce/mcp.json.\n",
        );
        return out;
    }
    if missing_config {
        out.push_str(
            "No .lapce/mcp.json in this workspace. Example:\n\n```json\n{\n  \"servers\": [\n    { \"name\": \"filesystem\", \"command\": \"npx\", \"args\": [\"-y\", \"@modelcontextprotocol/server-filesystem\", \".\"] }\n  ]\n}\n```\n\nStarting a server runs its command from the workspace root; only configure servers you trust - this file can arrive via git clone.\n",
        );
    }
    if reports.is_empty() && !missing_config {
        out.push_str("All configured servers are disabled.\n");
    }
    for report in reports {
        out.push_str(&format!("## {} - {}\n\n", report.name, report.status));
        if !report.command.is_empty() {
            out.push_str(&format!("`{}`\n\n", report.command));
        }
        for (name, description) in &report.tools {
            out.push_str(&format!("- `{name}` - {description}\n"));
        }
        if report.status == "connected" {
            out.push_str(
                "\nTool calls stay disabled until the Act review UI owns them.\n",
            );
        }
        out.push('\n');
    }
    out
}

/// Spawn, handshake, list tools; always reaps the child.
fn probe_server(server: &McpServer, root: &Path, timeout: Duration) -> ServerReport {
    let (tx, rx) = std::sync::mpsc::channel();
    let pid_slot = Arc::new(Mutex::new(None::<u32>));
    let slot = pid_slot.clone();
    let worker = server.clone();
    let name = server.name.clone();
    let command = server.command.clone();
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let report = probe_server_blocking(&worker, &root, &slot);
        let _ = tx.send(report);
    });
    match rx.recv_timeout(timeout) {
        Ok(report) => report,
        Err(_) => {
            if let Some(pid) = pid_slot.lock().unwrap().take() {
                let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
            }
            ServerReport {
                name,
                command,
                status: format!("timed out after {}s", timeout.as_secs()),
                tools: Vec::new(),
            }
        }
    }
}

fn probe_server_blocking(
    server: &McpServer,
    root: &Path,
    pid_slot: &Arc<Mutex<Option<u32>>>,
) -> ServerReport {
    let mut report = ServerReport {
        name: server.name.clone(),
        command: server.command.clone(),
        status: String::new(),
        tools: Vec::new(),
    };
    let args: Vec<&str> = server.args.iter().map(String::as_str).collect();
    let mut process = match AcpProcess::spawn(&server.command, &args, root) {
        Ok(p) => p,
        Err(e) => {
            report.status = format!("launch failed: {e}");
            return report;
        }
    };
    *pid_slot.lock().unwrap() = Some(process.id());
    let (mut session, init) = McpSession::start("lapce", env!("CARGO_PKG_VERSION"));
    let result = (|| -> Result<(), String> {
        process.send(&init).map_err(|e| e.message)?;
        let mut guard = 0;
        loop {
            guard += 1;
            if guard > HANDSHAKE_MESSAGE_CAP {
                return Err("handshake did not finish after 64 messages".into());
            }
            let Some(message) = process.recv().map_err(|e| e.message)? else {
                let logs = process.stderr_log();
                return Err(format!("server exited during setup. {}", logs.join(" | ")));
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
                return Ok(());
            }
        }
    })();
    match result {
        Ok(()) => {
            report.status = "connected".into();
            report.tools = session
                .tools()
                .iter()
                .map(|t| (t.name.clone(), t.description.clone()))
                .collect();
        }
        Err(e) => report.status = format!("error: {e}"),
    }
    process.kill();
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_covers_missing_config_and_connected_servers() {
        let closed = render_report(&[], false, false);
        assert!(closed.contains("Open a local project folder first"));

        let missing = render_report(&[], true, true);
        assert!(missing.contains("No .lapce/mcp.json"));
        assert!(missing.contains("git clone"));

        let report = ServerReport {
            name: "fileserv".into(),
            command: "npx -y @mcp/fs".into(),
            status: "connected".into(),
            tools: vec![("läs_fil".into(), "läser en fíl 🦀".into())],
        };
        let rendered = render_report(&[report], false, true);
        assert!(rendered.contains("## fileserv - connected"));
        assert!(rendered.contains("- `läs_fil` - läser en fíl 🦀"));
        assert!(rendered.contains("Tool calls stay disabled"));

        let disabled = render_report(&[], false, true);
        assert!(disabled.contains("All configured servers are disabled"));
    }
}
