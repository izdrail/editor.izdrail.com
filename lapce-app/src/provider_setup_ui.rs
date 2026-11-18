//! Explicit, pending-only setup choices. No config write or network operation here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setup { LocalCpu, Ollama, LmStudio }
impl Setup {
    pub fn label(self) -> &'static str {
        match self { Self::LocalCpu => "On-device CPU", Self::Ollama => "Ollama on this computer", Self::LmStudio => "LM Studio on this computer" }
    }
    pub fn pending(self) -> (&'static str, Option<&'static str>, &'static str) {
        match self {
            Self::LocalCpu => ("local", None, "Pending: on-device CPU. Remote endpoint/model kept for later; no download or request started. Click Apply to save."),
            Self::Ollama => ("ollama", Some("http://127.0.0.1:11434"), "Pending: Ollama on this computer. Start Ollama, enter an installed model ID, then Apply. No model download, discovery or test started."),
            Self::LmStudio => ("ollama", Some("http://127.0.0.1:1234"), "Pending: LM Studio on this computer. Start its API server, load a model, enter its model ID, then Apply. No request started."),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_setup_presets_are_loopback_and_pending_only() {
        assert_eq!(Setup::LocalCpu.pending().0, "local");
        assert_eq!(Setup::LocalCpu.pending().1, None);
        for (preset, endpoint) in [(Setup::Ollama, "http://127.0.0.1:11434"), (Setup::LmStudio, "http://127.0.0.1:1234")] {
            let (backend, target, hint) = preset.pending();
            assert_eq!(backend, "ollama"); assert_eq!(target, Some(endpoint));
            assert!(hint.starts_with("Pending:"));
            assert!(hint.contains("enter") && hint.contains("Apply"));
            assert!(crate::chat::completion_url(endpoint).is_ok());
            assert!(preset.label().contains("this computer"));
        }
    }
}
