//! Explicit tool-aware Ollama request. Hosted API execution is not available here.
use crate::{
    agent_protocol::AgentMessage,
    agent_wire::{self, Reply},
    chat::completion_url,
};
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
pub fn ollama(
    endpoint: &str,
    model: &str,
    messages: &[AgentMessage],
    tools: &[Value],
    max_output: u32,
    cancelled: Arc<AtomicBool>,
) -> Result<Reply, String> {
    let request = agent_wire::request(model, messages, tools, max_output)?;
    let allowed: Vec<&str> = tools
        .iter()
        .map(|v| {
            v.pointer("/function/name")
                .and_then(Value::as_str)
                .ok_or("Missing tool name")
        })
        .collect::<Result<_, _>>()?;
    let url = completion_url(endpoint)?;
    if cancelled.load(Ordering::Acquire) {
        return Err("Cancelled".into());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async {
        let run=async {
            let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).connect_timeout(std::time::Duration::from_secs(15)).timeout(std::time::Duration::from_secs(180)).build().map_err(|e|e.to_string())?;
            let mut response=crate::chat::with_auth(client.post(url).json(&request), endpoint).send().await.map_err(|e|e.to_string())?;
            if !response.status().is_success() {
                let status = response.status();
                // The provider's error body says WHY (unknown model,
                // unsupported tools, schema rejection); clip it into
                // the message so a 400 is debuggable from the panel.
                let body = response.text().await.unwrap_or_default();
                let detail: String = body.chars().take(500).collect();
                // Ollama names the model in a 400 when the model has
                // no tool support; lead with the actionable fix
                // (host QA: gemma3:4b Act runs died this way).
                if status.as_u16() == 401 || status.as_u16() == 403 {
                    return Err(format!(
                        "The provider rejected the request (HTTP {status}). Open Settings > AI to save an API key in the shared connection and Test connection. LAPCE_LLM_API_KEY overrides the saved key."
                    ));
                }
                if body.contains("does not support tools") {
                    return Err(format!(
                        "The model '{model}' does not support tools, so Act can't use it. Pick a tool-capable model (for example qwen3:4b) in the model dropdown. (Agent provider returned HTTP {status}: {detail})"
                    ));
                }
                return Err(format!(
                    "Agent provider returned HTTP {status}: {detail}"
                ));
            }
            let mut bytes=Vec::new();
            while let Some(chunk)=response.chunk().await.map_err(|e|e.to_string())? {
                if bytes.len()+chunk.len()>2*1024*1024 {return Err("Agent response too large".into());}bytes.extend_from_slice(&chunk);
            }
            let value=serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;
            agent_wire::reply(&value,&allowed)
        };
        tokio::select! {
            result=run=>result,
            _=async {loop {if cancelled.load(Ordering::Acquire) {break;}tokio::time::sleep(std::time::Duration::from_millis(25)).await;}}=>Err("Cancelled".into()),
            _=tokio::time::sleep(std::time::Duration::from_secs(180))=>Err("Agent request timed out".into()),
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::atomic::AtomicU64,
    };
    /// Read one HTTP request fully (headers + content-length body):
    /// an Act request with the contract and tool definitions runs
    /// several KB, past a single read.
    fn read_request(socket: &mut std::net::TcpStream) -> String {
        let _ = socket.set_read_timeout(Some(std::time::Duration::from_secs(
            5,
        )));
        let mut buf = Vec::new();
        let mut chunk = [0; 8192];
        loop {
            let n = match socket.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            buf.extend_from_slice(&chunk[..n]);
            if let Some(head_end) = buf
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
            {
                let head = String::from_utf8_lossy(&buf[..head_end]);
                let len = head
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if buf.len() >= head_end + 4 + len {
                    break;
                }
            }
            if buf.len() > 1024 * 1024 {
                break;
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }
    fn respond(socket: &mut std::net::TcpStream, body: &str) {
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    }
    fn body_of(request: &str) -> &str {
        request.split("\r\n\r\n").nth(1).unwrap_or("")
    }
    /// End to end (item 234): the loop drives a real Act wire
    /// conversation - the first request carries the contract and
    /// tool definitions, a tool reply follows, and the second
    /// request carries the tool result back to the provider.
    #[test]
    fn act_wire_round_trip_tool_call_then_proposal() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", server.local_addr().unwrap());
        let (req_tx, req_rx) = std::sync::mpsc::channel::<String>();
        let worker = std::thread::spawn(move || {
            let (mut s, _) = server.accept().unwrap();
            req_tx.send(read_request(&mut s)).unwrap();
            respond(
                &mut s,
                r#"{"choices":[{"finish_reason":"tool_calls","message":{"content":null,"tool_calls":[{"id":"c1","type":"function","function":{"name":"run_command","arguments":"{\"command\":\"ls -la\"}"}}]}}]}"#,
            );
            let (mut s, _) = server.accept().unwrap();
            req_tx.send(read_request(&mut s)).unwrap();
            respond(
                &mut s,
                r#"{"choices":[{"finish_reason":"stop","message":{"content":"{\"changes\":[{\"path\":\"a.txt\",\"old_text\":\"dirty\",\"new_text\":\"clean\"}]}"}}]}"#,
            );
        });
        let ep = endpoint.clone();
        let result = crate::agent_loop::run(
            crate::agent_loop::Mode::Act,
            "change the word dirty to clean",
            crate::agent_loop::Limits::default(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicU64::new(0)),
            move |messages, tools| {
                ollama(
                    &ep,
                    "test",
                    messages,
                    tools,
                    512,
                    Arc::new(AtomicBool::new(false)),
                )
            },
            |_| Ok("file-a\nfile-b".to_string()),
            |_| {},
            &[],
            Some("a.txt"),
        )
        .unwrap();
        worker.join().unwrap();
        assert!(result.contains("old_text"));
        let req1 = req_rx.recv().unwrap();
        let body1 = body_of(&req1);
        // The Act contract with the worked example naming the open
        // file, plus the tool definitions.
        assert!(body1.contains("\"role\":\"system\""));
        assert!(body1.contains("Example:"));
        assert!(body1.contains("a.txt"));
        assert!(body1.contains("run_command"));
        let req2 = req_rx.recv().unwrap();
        let body2 = body_of(&req2);
        // The tool result rides back as a tool-role message keyed by
        // the provider's call id.
        assert!(body2.contains("\"role\":\"tool\""));
        assert!(body2.contains("c1"));
        assert!(body2.contains("file-a"));
    }
    /// End to end (item 234): a prose first answer produces a second
    /// request carrying the repair message, not a run failure.
    #[test]
    fn act_wire_bad_answer_triggers_repair_request() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", server.local_addr().unwrap());
        let (req_tx, req_rx) = std::sync::mpsc::channel::<String>();
        let worker = std::thread::spawn(move || {
            let (mut s, _) = server.accept().unwrap();
            req_tx.send(read_request(&mut s)).unwrap();
            respond(
                &mut s,
                r#"{"choices":[{"finish_reason":"stop","message":{"content":"just some prose"}}]}"#,
            );
            let (mut s, _) = server.accept().unwrap();
            req_tx.send(read_request(&mut s)).unwrap();
            respond(
                &mut s,
                r#"{"choices":[{"finish_reason":"stop","message":{"content":"{\"changes\":[{\"path\":\"a.txt\",\"old_text\":\"dirty\",\"new_text\":\"clean\"}]}"}}]}"#,
            );
        });
        let ep = endpoint.clone();
        let result = crate::agent_loop::run(
            crate::agent_loop::Mode::Act,
            "change the word dirty to clean",
            crate::agent_loop::Limits::default(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicU64::new(0)),
            move |messages, tools| {
                ollama(
                    &ep,
                    "test",
                    messages,
                    tools,
                    512,
                    Arc::new(AtomicBool::new(false)),
                )
            },
            |_| Ok(String::new()),
            |_| {},
            &[],
            Some("a.txt"),
        )
        .unwrap();
        worker.join().unwrap();
        assert!(result.contains("old_text"));
        let _ = req_rx.recv().unwrap();
        let req2 = req_rx.recv().unwrap();
        assert!(body_of(&req2).contains("not a valid Act proposal"));
    }
    #[test]
    fn loopback_tool_reply_no_execution() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", server.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut s, _) = server.accept().unwrap();
            let mut buf = [0; 8192];
            let n = s.read(&mut buf).unwrap();
            assert!(
                std::str::from_utf8(&buf[..n])
                    .unwrap()
                    .contains("POST /v1/chat/completions")
            );
            let body = r#"{"choices":[{"finish_reason":"tool_calls","message":{"content":null,"tool_calls":[{"id":"call1","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"README.md\"}"}}]}}]}"#;
            write!(s,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        });
        let tool = serde_json::json!({"type":"function","function":{"name":"read_file","parameters":{"type":"object"}}});
        let r = ollama(
            &endpoint,
            "test",
            &[AgentMessage::User {
                content: "read".into(),
            }],
            &[tool],
            512,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert_eq!(r.calls[0].arguments["path"], "README.md");
        worker.join().unwrap();
    }
    #[test]
    fn http_error_surfaces_provider_body() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", server.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut s, _) = server.accept().unwrap();
            let mut buf = [0; 8192];
            let _ = s.read(&mut buf).unwrap();
            let body =
                r#"{"error":{"message":"model 'x' does not support tools"}}"#;
            write!(s,"HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        });
        let result = ollama(
            &endpoint,
            "x",
            &[AgentMessage::User {
                content: "hi".into(),
            }],
            &[],
            64,
            Arc::new(AtomicBool::new(false)),
        );
        worker.join().unwrap();
        let error = result.unwrap_err();
        assert!(error.contains("HTTP 400 Bad Request"));
        assert!(error.contains("does not support tools"));
    }

    #[test]
    fn no_tools_model_gets_actionable_error() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", server.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut s, _) = server.accept().unwrap();
            let mut buf = [0; 8192];
            let _ = s.read(&mut buf).unwrap();
            let body = r#"{"error":{"message":"registry.ollama.ai/library/gemma3:4b does not support tools"}}"#;
            write!(s,"HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        });
        let result = ollama(
            &endpoint,
            "gemma3:4b",
            &[AgentMessage::User {
                content: "hi".into(),
            }],
            &[],
            64,
            Arc::new(AtomicBool::new(false)),
        );
        worker.join().unwrap();
        let error = result.unwrap_err();
        assert!(error.contains("'gemma3:4b' does not support tools"));
        assert!(error.contains("tool-capable"));
    }

    #[test]
    fn pre_cancel_makes_no_request() {
        assert_eq!(
            ollama(
                "http://127.0.0.1:1",
                "test",
                &[AgentMessage::User {
                    content: "x".into()
                }],
                &[],
                10,
                Arc::new(AtomicBool::new(true))
            )
            .unwrap_err(),
            "Cancelled"
        );
    }
}
