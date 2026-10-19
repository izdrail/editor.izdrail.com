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
            let mut response=client.post(url).json(&request).send().await.map_err(|e|e.to_string())?;
            if !response.status().is_success() {
                let status = response.status();
                // The provider's error body says WHY (unknown model,
                // unsupported tools, schema rejection); clip it into
                // the message so a 400 is debuggable from the panel.
                let body = response.text().await.unwrap_or_default();
                let detail: String = body.chars().take(500).collect();
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
    };
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
