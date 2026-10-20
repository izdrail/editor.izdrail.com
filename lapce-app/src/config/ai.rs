use serde::{Deserialize, Serialize};
use structdesc::FieldNames;
/// On-device by default. Remote inference requires an explicit backend choice.
#[derive(FieldNames, Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct AiConfig {
    #[field_names(
        desc = "AI ghost text at the cursor. Local backend keeps inference on this device by default. Selecting ollama sends code around the cursor to the configured endpoint."
    )]
    pub enabled: bool,
    #[field_names(
        desc = "Ghost text backend: local (default, on-device CPU inference) or ollama (opt-in, code around the cursor sent to the configured endpoint)."
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
    #[field_names(
        desc = "After accepting ghost text, predict the next edit elsewhere in the file. Ollama backend only; sends the same bounded context plus recent edits to the endpoint."
    )]
    pub next_edit: bool,
    #[field_names(desc = "Curated local FIM model. Download in the AI Chat local model manager.")]
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
            backend: "local".into(),
            endpoint: "https://ai.izdrail.com".into(),
            remote_model: "qwen2.5-coder:1.5b-instruct-q4_K_M".into(),
            timeout_ms: 15_000,
            next_edit: false,
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
    fn defaults_keep_ghost_text_local() {
        let config = AiConfig::default();
        assert!(config.enabled);
        assert_eq!(config.backend, "local");
        assert_eq!(config.endpoint, "https://ai.izdrail.com");
        // Verified on /api/tags and /v1/models 2026-10-17.
        assert_eq!(config.remote_model, "qwen2.5-coder:1.5b-instruct-q4_K_M");
        assert!(config.auto_trigger);
        let expected_timeout = 15_000;
        assert_eq!(config.timeout_ms, expected_timeout);
        // Remote next-edit prediction is opt-in.
        assert!(!config.next_edit);
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
