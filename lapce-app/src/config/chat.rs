use serde::{Deserialize, Serialize};
use structdesc::FieldNames;
#[derive(FieldNames, Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct ChatConfig {
    #[field_names(
        desc = "Enable AI Chat. Local CPU is the default; Ollama sends context only on Send."
    )]
    pub enabled: bool,
    #[field_names(
        desc = "Save chat prompts and replies on this device. May contain private code; default off."
    )]
    pub save_history: bool,
    #[field_names(desc = "Backend: local (CPU Candle) or ollama (your server).")]
    pub backend: String,
    #[field_names(desc = "Curated local instruct model. Pick and download below.")]
    pub local_model: String,
    #[field_names(
        desc = "Ollama endpoint, root or /v1. No hosted service or API key required."
    )]
    pub endpoint: String,
    #[field_names(desc = "Ollama chat model name.")]
    pub model: String,
}
impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            save_history: false,
            backend: "local".into(),
            local_model: "qwen-chat-1.5b".into(),
            endpoint: "https://ai.izdrail.com".into(),
            model: "qwen2.5-coder:1.5b-instruct-q4_K_M".into(),
        }
    }
}
