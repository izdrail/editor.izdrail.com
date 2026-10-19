//! AI chat defaults. Stefan's directive (2026-10-19): the default backend
//! talks to his gateway at https://llm.izdrail.com (OpenAI-compatible /v1,
//! ~197 models, `auto` routing; set LAPCE_LLM_API_KEY - the gateway rejects
//! key-less calls with 401, and its Ollama emulation is disabled). His
//! key-less Ollama at ai.izdrail.com stays selectable; the local Candle
//! lane stays available as an opt-in. Pinned by the test at the bottom.
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
    #[field_names(
        desc = "Optional hosted provider configurations. Disabled by default; agent transport not yet enabled."
    )]
    pub hosted_providers: Vec<crate::agent_protocol::ProviderConfig>,
    #[field_names(
        desc = "Backend: local (CPU Candle) or ollama (any OpenAI-compatible /v1 server - llm.izdrail.com by default, or vLLM/LM Studio/llama.cpp)."
    )]
    pub backend: String,
    #[field_names(desc = "Curated local instruct model. Pick and download below.")]
    pub local_model: String,
    #[field_names(
        desc = "OpenAI-compatible endpoint, root or /v1. llm.izdrail.com is the default and needs LAPCE_LLM_API_KEY set in the environment."
    )]
    pub endpoint: String,
    #[field_names(desc = "Ollama chat model name.")]
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
            backend: "ollama".into(),
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
    fn defaults_point_at_the_llm_gateway() {
        let config = ChatConfig::default();
        assert_eq!(config.backend, "ollama");
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
