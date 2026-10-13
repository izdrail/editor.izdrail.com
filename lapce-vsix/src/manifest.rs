use anyhow::{Result, bail};
use serde_json::Value;

/// The parts of an extension `package.json` that matter for declarative import.
#[derive(Debug, Clone)]
pub struct Manifest {
    pub raw: Value,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub display_name: String,
    pub description: String,
    pub license: Option<String>,
    pub repository: Option<String>,
    /// Entry points (`main`, `browser`) that make this an extension with
    /// executable code. They are never loaded or run.
    pub code_entries: Vec<String>,
}

fn localize(value: &mut Value, nls: &Value) {
    match value {
        Value::String(s) => {
            if s.len() > 2 && s.starts_with('%') && s.ends_with('%') {
                let key = &s[1..s.len() - 1];
                match nls.get(key) {
                    Some(Value::String(t)) => *s = t.clone(),
                    Some(Value::Object(o)) => {
                        if let Some(Value::String(t)) = o.get("message") {
                            *s = t.clone();
                        }
                    }
                    _ => {}
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|v| localize(v, nls)),
        Value::Object(o) => o.values_mut().for_each(|v| localize(v, nls)),
        _ => {}
    }
}

impl Manifest {
    pub fn parse(json: &str, nls: Option<&str>) -> Result<Self> {
        let mut raw = crate::jsonc::parse(json)?;
        if let Some(nls) = nls.and_then(|n| crate::jsonc::parse(n).ok()) {
            localize(&mut raw, &nls);
        }
        let text = |k: &str| raw.get(k).and_then(Value::as_str).map(str::to_owned);
        let Some(name) = text("name") else {
            bail!("package.json has no name");
        };
        let publisher = text("publisher").unwrap_or_else(|| "unknown".into());
        let version = text("version").unwrap_or_else(|| "0.0.0".into());
        let mut code_entries = Vec::new();
        for k in ["main", "browser"] {
            if let Some(v) = text(k) {
                code_entries.push(format!("{k}: {v}"));
            }
        }
        let license = match raw.get("license") {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Object(o)) => {
                o.get("type").and_then(Value::as_str).map(str::to_owned)
            }
            _ => None,
        };
        let repository = match raw.get("repository") {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Object(o)) => {
                o.get("url").and_then(Value::as_str).map(str::to_owned)
            }
            _ => None,
        };
        Ok(Self {
            display_name: text("displayName").unwrap_or_else(|| name.clone()),
            description: text("description").unwrap_or_default(),
            name,
            publisher,
            version,
            license,
            repository,
            code_entries,
            raw,
        })
    }

    pub fn id(&self) -> String {
        format!("{}.{}", self.publisher, self.name)
    }

    pub fn has_code(&self) -> bool {
        !self.code_entries.is_empty()
    }

    /// Items under `contributes.<key>`.
    pub fn contributes(&self, key: &str) -> Vec<&Value> {
        self.raw
            .get("contributes")
            .and_then(|c| c.get(key))
            .and_then(Value::as_array)
            .map(|a| a.iter().collect())
            .unwrap_or_default()
    }
}
