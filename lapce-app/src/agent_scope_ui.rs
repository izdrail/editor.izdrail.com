//! Display-only mode scope. Labels do not grant tool or read permissions.
use crate::agent_loop::Mode;
pub fn scope_hint(enabled: bool, acp: bool, mode: Option<Mode>) -> &'static str {
    if !enabled { return "AI is off. Choose a connection in AI settings before sending."; }
    if acp { return "External agent: its runtime owns tools and permissions. Native review is separate."; }
    match mode {
        None => "Chat: conversation with attached context. No project tool loop.",
        Some(Mode::Ask) => "Ask: project read/search tools. No proposed file edits.",
        Some(Mode::Plan) => "Plan: project read/search tools and a saved plan artifact. No source edits.",
        Some(Mode::Act) => "Act: proposed edits require review. Commands and MCP tools require approval per call.",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agent_scope_labels_preserve_permission_boundaries() {
        for mode in [None, Some(Mode::Ask), Some(Mode::Plan), Some(Mode::Act)] {
            assert!(scope_hint(false, true, mode).starts_with("AI is off"));
            assert!(scope_hint(true, true, mode).contains("runtime owns"));
        }
        assert!(scope_hint(true, false, None).contains("No project tool loop"));
        assert!(scope_hint(true, false, Some(Mode::Ask)).contains("No proposed file edits"));
        assert!(scope_hint(true, false, Some(Mode::Plan)).contains("No source edits"));
        let act = scope_hint(true, false, Some(Mode::Act));
        assert!(act.contains("require review") && act.contains("approval per call"));
    }
}
