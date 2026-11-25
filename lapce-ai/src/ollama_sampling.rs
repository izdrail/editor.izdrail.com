//! Optional native Ollama profiles. No Request/RemoteFim wire fields change.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeat_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f64>,
}
impl Config {
    pub fn parse(value: &str) -> Result<Self> {
        let config: Self = serde_json::from_str(value)?;
        ensure!(config.temperature.is_none_or(|v| v.is_finite() && (0.0..=5.0).contains(&v)), "Ollama temperature must be 0..5");
        ensure!(config.top_k.is_none_or(|v| (1..=1_000_000).contains(&v)), "Ollama top_k must be positive");
        ensure!(config.top_p.is_none_or(|v| v.is_finite() && v > 0.0 && v <= 1.0), "Ollama top_p must be >0 and <=1");
        ensure!(config.min_p.is_none_or(|v| v.is_finite() && (0.0..=1.0).contains(&v)), "Ollama min_p must be 0..1");
        ensure!(config.repeat_penalty.is_none_or(|v| v.is_finite() && (0.01..=100.0).contains(&v)), "Ollama repeat_penalty must be 0.01..100");
        ensure!([config.frequency_penalty, config.presence_penalty].iter().all(|v| v.is_none_or(|v| v.is_finite() && (-2.0..=2.0).contains(&v))), "Ollama penalties must be -2..2");
        Ok(config)
    }
    pub fn apply(&self, body: &mut serde_json::Value) -> Result<()> {
        let options = body.get_mut("options").and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| anyhow::anyhow!("Missing Ollama options object"))?;
        let overrides = serde_json::to_value(self)?;
        for (key, value) in overrides.as_object().unwrap() {
            options.insert(key.clone(), value.clone());
        }
        Ok(())
    }
}
pub fn apply_environment(chat: bool, body: &mut serde_json::Value) -> Result<()> {
    let name = if chat { "LAPCE_AI_OLLAMA_CHAT_SAMPLING" } else { "LAPCE_AI_OLLAMA_FIM_SAMPLING" };
    match std::env::var(name) {
        Ok(value) => Config::parse(&value)?.apply(body),
        Err(std::env::VarError::NotPresent) => Ok(()),
        Err(error) => Err(error.into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_unknown_and_budget_stop_overrides() {
        for input in [r#"{"num_predict":1}"#, r#"{"stop":[]}"#, r#"{"stop_token_ids":[1]}"#, r#"{"temperature":-1}"#, r#"{"top_k":0}"#, r#"{"top_p":0}"#, r#"{"min_p":2}"#, r#"{"repeat_penalty":0}"#, r#"{"presence_penalty":3}"#] {
            assert!(Config::parse(input).is_err(), "{input}");
        }
    }
    #[test]
    fn merges_supported_options_without_replacing_fim_safety() {
        let mut body = crate::ollama::fim_body("model", "prefix", "suffix", 32);
        let before = body.clone();
        Config::parse("{}").unwrap().apply(&mut body).unwrap();
        assert_eq!(body, before);
        Config::parse(r#"{"temperature":0.7,"top_k":20,"top_p":0.9,"min_p":0.1,"seed":42,"repeat_penalty":1.1,"frequency_penalty":0.2,"presence_penalty":0.3}"#).unwrap().apply(&mut body).unwrap();
        assert_eq!(body["options"]["num_predict"], before["options"]["num_predict"]);
        assert_eq!(body["options"]["stop"], before["options"]["stop"]);
        assert_eq!(body["options"]["seed"], 42);
        assert_eq!(body["options"]["temperature"], 0.7);
        assert_eq!(body["raw"], true);
    }
}
