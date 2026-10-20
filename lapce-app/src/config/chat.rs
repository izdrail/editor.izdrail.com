//! Chat inference is local by default. Remote backend/provider use is opt-in.
use serde::{Deserialize, Serialize};
use structdesc::FieldNames;
#[derive(FieldNames, Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct ChatConfig {
    #[field_names(
        desc = "Enable AI Chat. Local on-device CPU inference is the default; selecting ollama sends context on Send."
    )]
    pub enabled: bool,
    #[field_names(
        desc = "Save chat prompts and replies on this device. May contain private code; default off."
    )]
    pub save_history: bool,
    #[field_names(
        desc = "Optional hosted provider configurations. Disabled by default; agent transport not yet enabled."
    )]
    pub hosted_providers: Vec<crate::agent_protocol::ProviderConfig>,
    #[field_names(
        desc = "Backend: local (default, CPU Candle) or ollama (opt-in) (any OpenAI-compatible /v1 server - llm.izdrail.com by default, or vLLM/LM Studio/llama.cpp)."
    )]
    pub backend: String,
    #[field_names(desc = "Use the shared AI connection for ghost text after explicit Apply. Legacy choices remain stored.")]
    pub shared_connection: bool,
    #[field_names(desc = "Curated local instruct model. Download in the AI Chat local model manager.")]
    pub local_model: String,
    #[field_names(
        desc = "OpenAI-compatible endpoint, root or /v1. llm.izdrail.com is the default. Save its API key in the LLM connection section above; LAPCE_LLM_API_KEY is an optional override."
    )]
    pub endpoint: String,
    #[field_names(desc = "Chat model name or routing alias (auto by default).")]
    pub model: String,
    #[field_names(
        desc = "ACP agent command for the chat panel, e.g. `opencode acp` or a Claude Code/Codex adapter binary. Empty disables ACP mode."
    )]
    pub acp_command: String,
}
impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            save_history: false,
            hosted_providers: vec![
                crate::agent_protocol::ProviderConfig::hosted(
                    crate::agent_protocol::ProviderKind::Anthropic,
                ),
                crate::agent_protocol::ProviderConfig::hosted(
                    crate::agent_protocol::ProviderKind::OpenAi,
                ),
                crate::agent_protocol::ProviderConfig::hosted(
                    crate::agent_protocol::ProviderKind::OpenRouter,
                ),
            ],
            backend: "local".into(),
            shared_connection: false,
            local_model: "qwen-chat-1.5b".into(),
            endpoint: "https://llm.izdrail.com".into(),
            model: "auto".into(),
            acp_command: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_chat_local_and_hosted_providers_disabled() {
        let config = ChatConfig::default();
        assert_eq!(config.backend, "local");
        assert!(!config.shared_connection);
        assert_eq!(config.endpoint, "https://llm.izdrail.com");
        // The gateway's routing alias (verified 2026-10-08); /v1
        // re-probed 2026-10-19: validated 401 without
        // LAPCE_LLM_API_KEY, Ollama emulation disabled.
        assert_eq!(config.model, "auto");
        // The key-less servers must not be mistaken for hosted providers:
        // hosted lanes stay present-but-disabled by default.
        assert!(config.hosted_providers.iter().all(|p| !p.enabled));
    }
}
