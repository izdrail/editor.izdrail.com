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

/// Draft-only suggestions. Picking one never changes mode or sends a request.
pub const STARTERS: &[(&str, &str)] = &[
    ("Explain this file", "Explain the current file and its main responsibilities."),
    ("Review for bugs", "Review the current file for bugs and edge cases. Explain findings before proposing changes."),
    ("Suggest tests", "Suggest focused tests for the current file, including failure cases."),
];
#[cfg(test)]
mod starter_tests {
    #[test]
    fn agent_starters_are_plain_drafts_not_slash_commands() {
        for (label, prompt) in super::STARTERS {
            assert!(!label.is_empty()); assert!(prompt.len() < 160);
            assert!(!prompt.starts_with('/')); assert!(prompt.contains("current file"));
        }
    }
}

/// Display only: keep source paths and context payloads unchanged.
/// Outside-workspace paths stay absolute so a different file is not disguised.
pub fn context_label(context: &crate::chat::Context, root: Option<&std::path::Path>) -> String {
    let path = std::path::Path::new(&context.name);
    let display = root.and_then(|root| path.strip_prefix(root).ok())
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or(path);
    format!("{}: {} | {} bytes{}",
        if context.selection { "Selection" } else { "File" },
        display.display(), context.text.len(),
        if context.truncated { " (truncated)" } else { "" })
}

#[cfg(test)]
mod context_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn agent_context_label_is_relative_without_changing_payload() {
        let context = crate::chat::Context::new("/project/src/User.php".into(), "abc", false);
        assert_eq!(context_label(&context, Some(Path::new("/project"))), "File: src/User.php | 3 bytes");
        assert_eq!(context.name, "/project/src/User.php");
        assert!(context.indicator().contains("/project/src/User.php"));
        assert_eq!(context_label(&context, None), context.indicator());
    }

    #[test]
    fn agent_context_label_keeps_external_paths_and_selection_limits() {
        let context = crate::chat::Context::new("/project-other/café.php".into(), &"é".repeat(7000), true);
        assert_eq!(context_label(&context, Some(Path::new("/project"))), context.indicator());
        assert!(context_label(&context, Some(Path::new("/project-other"))).starts_with("Selection: café.php"));
        assert!(context_label(&context, None).ends_with("12000 bytes (truncated)"));
        let scratch = crate::chat::Context::new("Untitled-1".into(), "", false);
        assert_eq!(context_label(&scratch, Some(Path::new("/project"))), "File: Untitled-1 | 0 bytes");
    }
}

/// Composer-only, explicit draft shortcuts. No send or mode change.
pub fn starter_shortcut(key: &floem::keyboard::Key, modifiers: floem::keyboard::Modifiers) -> Option<&'static str> {
    use floem::keyboard::{Key, Modifiers};
    if modifiers != Modifiers::ALT { return None; }
    let Key::Character(key) = key else { return None; };
    let index = match key.as_str() { "1" => 0, "2" => 1, "3" => 2, _ => return None };
    Some(STARTERS[index].1)
}
#[cfg(test)]
mod shortcut_tests {
    use super::*;
    use floem::keyboard::{Key, NamedKey, Modifiers};
    #[test]
    fn agent_starter_shortcuts_are_alt_digits_only() {
        for (index, digit) in ["1", "2", "3"].into_iter().enumerate() {
            let key = Key::Character(digit.into());
            assert_eq!(starter_shortcut(&key, Modifiers::ALT), Some(STARTERS[index].1));
            for mods in [Modifiers::empty(), Modifiers::CONTROL, Modifiers::ALT | Modifiers::SHIFT] {
                assert_eq!(starter_shortcut(&key, mods), None);
            }
        }
        assert_eq!(starter_shortcut(&Key::Named(NamedKey::Enter), Modifiers::ALT), None);
        assert_eq!(starter_shortcut(&Key::Character("4".into()), Modifiers::ALT), None);
    }
}


/// Blank drafts are inert; stopping running work never needs draft text.
pub fn composer_action_enabled(busy: bool, input: &str) -> bool {
    busy || !input.trim().is_empty()
}

#[cfg(test)]
mod composer_action_tests {
    #[test]
    fn agent_composer_blank_draft_is_inert_but_stop_is_always_available() {
        for draft in ["", " ", "\n\t", "\u{2003}"] {
            assert!(!super::composer_action_enabled(false, draft));
            assert!(super::composer_action_enabled(true, draft));
        }
        for draft in ["Hello", " /ask explain this ", "é"] {
            assert!(super::composer_action_enabled(false, draft));
            assert!(super::composer_action_enabled(true, draft));
        }
    }
}
