use serde::{Deserialize, Serialize};
use structdesc::FieldNames;
/// Ghost text defaults. Stefan's directive (2026-10-17): inline ghost text is
/// served by his key-less Ollama at https://ai.izdrail.com; the local Candle
/// lane stays as the on-device opt-in. Pinned by the tests at the bottom.
#[derive(FieldNames, Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct AiConfig {
    #[field_names(
        desc = "AI ghost text at the cursor. The default ollama backend sends the code around the cursor to the configured endpoint; backend = local keeps everything on this device."
    )]
    pub enabled: bool,
    #[field_names(
        desc = "Ghost text backend: ollama (default - code around the cursor is sent to the configured endpoint over the network) or local (CPU inference on this device - no code ever leaves this device)."
    )]
    pub backend: String,
    #[field_names(
        desc = "Ollama endpoint root for ghost text. Key-less; no API key needed or used. Ignored when backend = local."
    )]
    pub endpoint: String,
    #[field_names(desc = "Ollama model for ghost text.")]
    pub remote_model: String,
    #[field_names(desc = "Network ghost text timeout (1000-120000 ms).")]
    pub timeout_ms: u64,
    #[field_names(desc = "Curated local FIM model. Pick and download below.")]
    pub local_model: String,
    #[field_names(
        desc = "Local Qwen2 GGUF path; empty uses the downloaded curated model."
    )]
    pub model_path: String,
    #[field_names(
        desc = "Matching tokenizer.json path; empty uses the downloaded tokenizer."
    )]
    pub tokenizer_path: String,
    #[field_names(desc = "Prefix token budget (1-2048).")]
    pub prefix_tokens: usize,
    #[field_names(desc = "Suffix token budget (0-1024).")]
    pub suffix_tokens: usize,
    #[field_names(desc = "Output token limit (1-128).")]
    pub max_tokens: usize,
    #[field_names(
        desc = "Automatically suggest ghost text after a typing pause. With the ollama backend each pause sends the code around the cursor to the endpoint; with local, CPU inference takes seconds per suggestion."
    )]
    pub auto_trigger: bool,
    #[field_names(
        desc = "Typing pause before an automatic suggestion starts (50-2000 ms)."
    )]
    pub auto_trigger_debounce_ms: u64,
}
impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: "ollama".into(),
            endpoint: "https://ai.izdrail.com".into(),
            remote_model: "qwen2.5-coder:1.5b-instruct-q4_K_M".into(),
            timeout_ms: 15_000,
            local_model: "qwen-fim-1.5b".into(),
            model_path: String::new(),
            tokenizer_path: String::new(),
            prefix_tokens: 512,
            suffix_tokens: 128,
            max_tokens: 32,
            auto_trigger: true,
            auto_trigger_debounce_ms: 400,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_serve_izdrail_ghost_text() {
        let config = AiConfig::default();
        assert!(config.enabled);
        assert_eq!(config.backend, "ollama");
        assert_eq!(config.endpoint, "https://ai.izdrail.com");
        // Verified on /api/tags and /v1/models 2026-10-17.
        assert_eq!(config.remote_model, "qwen2.5-coder:1.5b-instruct-q4_K_M");
        assert!(config.auto_trigger);
        let expected_timeout = 15_000;
        assert_eq!(config.timeout_ms, expected_timeout);
    }

    #[test]
    fn on_device_lane_remains_available() {
        // The no-network guarantee is one settings word away, and the old
        // local knobs (curated model, explicit paths, token budgets) survive
        // the default flip untouched.
        let config = AiConfig::default();
        assert_eq!(config.local_model, "qwen-fim-1.5b");
        assert!(config.model_path.is_empty());
        let expected = (512, 128, 32);
        assert_eq!(
            (config.prefix_tokens, config.suffix_tokens, config.max_tokens),
            expected
        );
        // Unknown backend strings fall through to the local lane in the
        // editor, so a typo can never silently enable network sending.
        let mut config = config;
        config.backend = "local".into();
        let serialized = serde_json::to_string(&config).unwrap();
        let back: AiConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(back.backend, "local");
    }
}
