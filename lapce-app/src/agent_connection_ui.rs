//! Short, secret-free routing summary for the Agent panel.
fn label(value: &str) -> String {
    let mut text: String = value.chars().filter(|c| !c.is_control()).take(64).collect();
    if value.chars().count() > 64 { text.push_str("..."); }
    if text.trim().is_empty() { "Choose a model".into() } else { text }
}

pub fn connection_summary(enabled: bool, acp: bool, backend: &str, model: &str, local_model: &str) -> String {
    if !enabled { return "AI is off. Open AI settings to enable it.".into(); }
    if acp { return "External agent. Its own process manages models and sign-in.".into(); }
    match backend {
        "local" => format!("Local CPU | {}", label(local_model)),
        "ollama" => format!("Endpoint | {}", label(model)),
        _ => "Unknown backend. Open AI settings to choose a connection.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agent_connection_summary_distinguishes_routes_and_does_not_claim_connectivity() {
        assert_eq!(connection_summary(true, false, "local", "remote", "curated"), "Local CPU | curated");
        assert_eq!(connection_summary(true, false, "ollama", "qwen", "local"), "Endpoint | qwen");
        assert!(connection_summary(false, true, "ollama", "qwen", "local").contains("AI is off"));
        assert!(connection_summary(true, true, "ollama", "qwen", "local").contains("External agent"));
        assert!(connection_summary(true, false, "bad", "qwen", "local").contains("Unknown backend"));
    }
    #[test]
    fn agent_connection_summary_bounds_model_labels_and_strips_controls() {
        assert!(connection_summary(true, false, "ollama", "", "").contains("Choose a model"));
        let text = connection_summary(true, false, "ollama", &format!("\n\t{}", "x".repeat(200)), "");
        assert!(text.len() < 90);
        assert!(!text.contains('\n') && !text.contains('\t'));
    }
}
