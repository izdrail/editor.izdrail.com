//! VS Code snippet files (`.json` / `.code-snippets`).
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snippet {
    pub name: String,
    pub prefix: Vec<String>,
    /// Body in LSP snippet syntax, lines joined by `\n`.
    pub body: String,
    pub description: String,
    /// Language ids from a global snippet's `scope`; empty means "the file's
    /// language" (or all languages for a global snippet with no scope).
    pub scope: Vec<String>,
}

fn strings(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

pub fn parse(doc: &Value) -> Vec<Snippet> {
    let Some(map) = doc.as_object() else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(name, v)| {
            let body = match v.get("body")? {
                Value::String(s) => s.clone(),
                Value::Array(a) => a
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => return None,
            };
            let prefix = strings(v.get("prefix"));
            if prefix.is_empty() {
                return None;
            }
            Some(Snippet {
                name: name.clone(),
                prefix,
                body,
                description: strings(v.get("description")).join(" "),
                scope: v
                    .get("scope")
                    .and_then(Value::as_str)
                    .map(|s| s.split(',').map(|s| s.trim().to_owned()).collect())
                    .unwrap_or_default(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_prefix_and_multiline_body() {
        let doc = crate::jsonc::parse(
            r#"{"For":{"prefix":["for","fori"],"body":["for ($1) {","\t$0","}"],"description":"loop"},"x":{"body":"no prefix"}}"#,
        )
        .unwrap();
        let s = parse(&doc);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].prefix, ["for", "fori"]);
        assert_eq!(s[0].body, "for ($1) {\n\t$0\n}");
    }
}
