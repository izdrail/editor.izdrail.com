//! Presets are overlays: common + OS + compact + selected preset + user overrides.
//! They never replace the Vim Normal/Visual/operator maps or terminal controls.
pub const NAMES: &[&str] = &[
    "compact-65",
    "vscode",
    "phpstorm",
    "sublime",
    "notepad-plus-plus",
    "lapce",
];
/// One-line description shown next to the name in the switcher palette.
pub fn describe(name: &str) -> &'static str {
    match name {
        "compact-65" => "no F-keys needed, Ctrl+Alt navigation (default)",
        "vscode" => "Visual Studio Code style",
        "phpstorm" => "PhpStorm / JetBrains style",
        "sublime" => "Sublime Text style",
        "notepad-plus-plus" => "Notepad++ style",
        "lapce" => "unchanged upstream Lapce defaults",
        _ => "",
    }
}

/// Name the loader will actually use for `configured` (unknown names fall
/// back to the default).
pub fn effective(configured: &str) -> &str {
    NAMES.iter().copied().find(|n| *n == configured).unwrap_or(NAMES[0])
}

pub const COMPACT: &str =
    include_str!("../../../defaults/keymap-presets/compact.toml");

pub fn selected(name: &str) -> Option<&'static str> {
    match name {
        "lapce" => None,
        "vscode" => {
            Some(include_str!("../../../defaults/keymap-presets/vscode.toml"))
        }
        "phpstorm" => Some(include_str!(
            "../../../defaults/keymap-presets/phpstorm.toml"
        )),
        "sublime" => {
            Some(include_str!("../../../defaults/keymap-presets/sublime.toml"))
        }
        "notepad-plus-plus" => Some(include_str!(
            "../../../defaults/keymap-presets/notepad-plus-plus.toml"
        )),
        // A typo or an older config must leave essential commands usable.
        _ => Some(include_str!(
            "../../../defaults/keymap-presets/compact-65.toml"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command::lapce_internal_commands,
        keypress::{keymap::{KeyMap, KeyMapPress}, loader::KeyMapLoader},
    };
    use lapce_core::mode::Modes;

    fn loaded(name: &str, modal: bool) -> KeyMapLoader {
        let mut loader = KeyMapLoader::new();
        loader
            .load_from_str(super::super::DEFAULT_KEYMAPS_COMMON, modal)
            .unwrap();
        loader
            .load_from_str(super::super::DEFAULT_KEYMAPS_NONMACOS, modal)
            .unwrap();
        if let Some(preset) = selected(name) {
            loader.load_from_str(COMPACT, modal).unwrap();
            loader.load_from_str(preset, modal).unwrap();
        }
        loader
    }

    #[test]
    fn every_preset_is_described_and_unknown_falls_back() {
        for n in NAMES {
            assert!(!describe(n).is_empty(), "{n}");
            assert_eq!(effective(n), *n);
        }
        assert_eq!(effective("vsocde"), "compact-65");
        assert_eq!(effective(""), "compact-65");
        assert_eq!(describe("nope"), "");
    }

    #[test]
    fn presets_use_valid_commands_modes_conditions_and_keys() {
        let commands = lapce_internal_commands();
        for name in NAMES.iter().copied().filter(|name| *name != "lapce") {
            for text in [COMPACT, selected(name).unwrap()] {
                let mut loader = KeyMapLoader::new();
                loader.load_from_str(text, true).unwrap();
                let (_, command_maps) = loader.finalize();
                for (command, maps) in command_maps {
                    assert!(commands.contains_key(&command), "{name}: {command}");
                    for map in maps {
                        assert!(!map.key.is_empty());
                        assert!(map.modes == Modes::INSERT || map.modes.is_empty());
                        assert!(matches!(
                            map.when.as_deref(),
                            Some(
                                "editor_focus && !list_focus"
                                    | "file_explorer_focus"
                            )
                        ));
                    }
                }
            }
        }
    }

    #[test]
    fn every_preset_has_fn_free_navigation_and_editing() {
        for name in NAMES.iter().copied().filter(|name| *name != "lapce") {
            let (_, commands) = loaded(name, false).finalize();
            for (key, command) in [
                ("ctrl+alt+a", "line_start_non_blank"),
                ("ctrl+alt+e", "line_end"),
                ("ctrl+alt+u", "page_up"),
                ("ctrl+alt+d", "page_down"),
                ("ctrl+alt+b", "document_start"),
                ("ctrl+alt+f", "document_end"),
                ("ctrl+alt+backspace", "delete_forward"),
                ("ctrl+alt+w", "delete_word_forward"),
                ("ctrl+alt+n", "rename_symbol"),
                ("ctrl+alt+g", "goto_definition"),
                ("ctrl+alt+.", "next_error"),
                ("ctrl+alt+,", "previous_error"),
            ] {
                assert!(
                    commands[command]
                        .iter()
                        .any(|map| map.key == KeyMapPress::parse(key)),
                    "{name}: {command}"
                );
            }
        }
    }

    /// A command bound to a function key must also have a key without one,
    /// in the same scope, in every Fn-free preset (65% keyboards have no F row).
    #[test]
    fn no_command_is_reachable_only_through_a_function_key() {
        fn has_f_key(map: &KeyMap) -> bool {
            map.key.iter().any(|p| {
                let s = p.to_string();
                let last = s.rsplit('+').next().unwrap_or("");
                last.len() >= 2
                    && last.len() <= 3
                    && last.starts_with(['F', 'f'])
                    && last[1..].chars().all(|c| c.is_ascii_digit())
            })
        }
        // OS-level window close; the editor has its own close commands.
        let exempt = ["close_window"];
        for name in NAMES.iter().copied().filter(|name| *name != "lapce") {
            let (_, commands) = loaded(name, false).finalize();
            for (command, maps) in &commands {
                if exempt.contains(&command.as_str()) {
                    continue;
                }
                for f in maps.iter().filter(|m| has_f_key(m)) {
                    let explorer = |m: &KeyMap| m.when.as_deref().is_some_and(|w| w.contains("file_explorer_focus") && !w.contains("!file_explorer_focus"));
                    let ok = maps.iter().any(|m| {
                        !has_f_key(m)
                            && explorer(m) == explorer(f)
                            && (m.modes.is_empty() || m.modes.contains(Modes::INSERT))
                    });
                    assert!(ok, "{name}: {command} is bound only to a function key ({:?})", f.key.iter().map(|k| k.to_string()).collect::<Vec<_>>());
                }
            }
        }
    }

    #[test]
    fn selected_preset_wins_and_user_override_remains_last() {
        for (name, key, command) in [
            ("vscode", "ctrl+d", "select_next_current"),
            ("phpstorm", "ctrl+d", "duplicate_line_down"),
            ("phpstorm", "ctrl+y", "delete_line"),
            ("sublime", "ctrl+shift+d", "duplicate_line_down"),
            ("sublime", "ctrl+r", "palette.symbol"),
            ("notepad-plus-plus", "ctrl+shift+l", "delete_line"),
        ] {
            let mut loader = loaded(name, false);
            let (keys, _) = loaded(name, false).finalize();
            assert_eq!(
                keys[&KeyMapPress::parse(key)].last().unwrap().command,
                command
            );
            loader.load_from_str(&format!("[[keymaps]]\nkey = \"{key}\"\ncommand = \"undo\"\nmode = \"i\"\nwhen = \"editor_focus && !list_focus\"\n"), false).unwrap();
            let (keys, _) = loader.finalize();
            assert_eq!(
                keys[&KeyMapPress::parse(key)].last().unwrap().command,
                "undo"
            );
        }
    }

    #[test]
    fn vim_normal_bindings_survive_and_lapce_opt_out_is_available() {
        let (_, baseline) = loaded("lapce", true).finalize();
        for name in NAMES {
            let (_, commands) = loaded(name, true).finalize();
            for (command, maps) in &baseline {
                for map in maps.iter().filter(|map| {
                    !map.modes.is_empty()
                        && !map.modes.contains(Modes::INSERT)
                        && !map.modes.contains(Modes::TERMINAL)
                }) {
                    assert!(commands[command].contains(map), "{name}: {command}");
                }
            }
        }
        assert!(selected("lapce").is_none());
        assert_eq!(selected("typo"), selected("compact-65"));
    }
}
