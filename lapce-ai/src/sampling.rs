//! Explicit Candle sampling configuration. Absent config preserves legacy greedy.
use anyhow::{Result, ensure};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub temperature: f64,
    #[serde(default)]
    pub top_k: Option<usize>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub seed: u64,
}
impl Config {
    pub fn parse(value: &str) -> Result<Self> {
        let config: Self = serde_json::from_str(value)?;
        ensure!(config.temperature.is_finite() && (0.0..=5.0).contains(&config.temperature), "Candle temperature must be 0..5");
        ensure!(config.top_k.is_none_or(|k| (1..=1_000_000).contains(&k)), "Candle top_k must be positive");
        ensure!(config.top_p.is_none_or(|p| p.is_finite() && p > 0.0 && p <= 1.0), "Candle top_p must be >0 and <=1");
        ensure!(config.temperature > 0.0 || (config.top_k.is_none() && config.top_p.is_none()), "Greedy sampling does not accept top_k/top_p");
        Ok(config)
    }
    pub fn processor(&self, vocab: usize) -> Result<Option<LogitsProcessor>> {
        ensure!(self.top_k.is_none_or(|k| k <= vocab), "Candle top_k exceeds vocabulary");
        if self.temperature == 0.0 { return Ok(None); }
        let sampling = match (self.top_k, self.top_p) {
            (Some(k), Some(p)) => Sampling::TopKThenTopP { k, p, temperature:self.temperature },
            (Some(k), None) => Sampling::TopK { k, temperature:self.temperature },
            (None, Some(p)) => Sampling::TopP { p, temperature:self.temperature },
            (None, None) => Sampling::All { temperature:self.temperature },
        };
        Ok(Some(LogitsProcessor::from_sampling(self.seed, sampling)))
    }
}

pub fn from_environment(chat: bool) -> Result<Option<Config>> {
    // Separate optional profiles, no ambient file or model-card defaults.
    let name = if chat { "LAPCE_AI_CANDLE_CHAT_SAMPLING" } else { "LAPCE_AI_CANDLE_FIM_SAMPLING" };
    match std::env::var(name) {
        Ok(value) => Ok(Some(Config::parse(&value)?)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_and_unsupported_settings() {
        for value in [r#"{"temperature":-1}"#, r#"{"top_k":0}"#, r#"{"temperature":1,"top_p":0}"#, r#"{"min_p":0.1}"#, r#"{"temperature":0,"top_k":3}"#] {
            assert!(Config::parse(value).is_err(), "{value}");
        }
        assert!(Config::parse(r#"{"temperature":1,"top_k":3}"#).unwrap().processor(2).is_err());
        assert!(Config::parse("{}").unwrap().processor(2).unwrap().is_none());
    }
    #[test]
    fn seeded_sampling_is_reproducible() {
        let config = Config::parse(r#"{"temperature":0.7,"top_k":3,"top_p":0.9,"seed":42}"#).unwrap();
        let mut first = config.processor(4).unwrap().unwrap();
        let mut second = config.processor(4).unwrap().unwrap();
        let logits = candle_core::Tensor::new(&[0.1f32,0.2,0.3,0.4], &candle_core::Device::Cpu).unwrap();
        for _ in 0..20 { assert_eq!(first.sample(&logits).unwrap(), second.sample(&logits).unwrap()); }
    }
}
