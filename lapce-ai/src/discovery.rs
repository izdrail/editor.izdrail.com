//! Explicit, bounded Ollama discovery. Never pulls a model.
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::{io::Read, time::Duration};

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct Model {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub digest: String,
}

#[derive(Deserialize)]
struct Models {
    models: Vec<Model>,
}

pub fn discover(endpoint: &str, model: Option<&str>) -> Result<serde_json::Value> {
    let root = endpoint.trim().trim_end_matches('/');
    let root = root.strip_suffix("/v1").unwrap_or(root);
    ensure!(root.starts_with("http://") || root.starts_with("https://"), "Ollama endpoint must start with http:// or https://");
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(10))
        .timeout_write(Duration::from_secs(10))
        .redirects(0).build();
    let response = if let Some(model) = model {
        ensure!(!model.trim().is_empty(), "Ollama model name is empty");
        agent.post(&format!("{root}/api/show"))
            .send_json(serde_json::json!({"model":model}))?
    } else {
        agent.get(&format!("{root}/api/tags")).call()?
    };
    let mut bytes = Vec::new();
    response.into_reader().take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1024 * 1024, "Ollama discovery response exceeds limit");
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    ensure!(value.get("error").is_none(), "Ollama discovery returned a server error");
    if model.is_none() {
        let models: Models = serde_json::from_value(value)?;
        return Ok(serde_json::to_value(models.models)?);
    }
    ensure!(value.is_object(), "Ollama model information is not an object");
    // Preserve server-reported fields. In particular, no model family is
    // promoted to a client-side claim of FIM or tool support.
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::{BufRead, BufReader, Write}, net::TcpListener};

    #[test]
    fn discovery_is_explicit_and_never_pulls() {
        for (model, body, expected) in [
            (None, r#"{"models":[{"name":"test:1","size":42,"digest":"sha"}]}"#, "GET /api/tags"),
            (Some("test:1"), r#"{"capabilities":["completion"],"model_info":{"test.context_length":2048}}"#, "POST /api/show"),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}",listener.local_addr().unwrap());
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                let mut first = String::new();reader.read_line(&mut first).unwrap();
                assert!(first.starts_with(expected), "{first}");
                let mut length = 0;
                loop {
                    let mut line = String::new();reader.read_line(&mut line).unwrap();
                    if line == "\r\n" { break; }
                    if let Some((key, value)) = line.split_once(':') {
                        if key.eq_ignore_ascii_case("content-length") { length = value.trim().parse().unwrap(); }
                    }
                }
                let mut request = vec![0; length];reader.read_exact(&mut request).unwrap();
                if model.is_some() { assert_eq!(serde_json::from_slice::<serde_json::Value>(&request).unwrap()["model"], "test:1"); }
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            });
            let result = discover(&endpoint, model).unwrap();
            if model.is_none() { assert_eq!(result[0]["name"], "test:1"); }
            else { assert_eq!(result["capabilities"][0], "completion"); }
            server.join().unwrap();
        }
        assert!(discover("file:///tmp/local", None).is_err());
    }
}
