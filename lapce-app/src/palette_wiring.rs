//! Wiring checks for palettes. A new palette needs the same handful of edits
//! in several files; forgetting one compiles fine and only shows up at run
//! time. These tests make that a test failure:
//!
//! - every `PaletteKind` has a command, or is listed in `NO_COMMAND` on
//!   purpose (palettes opened by a prefix typed in the palette input);
//! - that command has a display message, a unique serialized name, and is
//!   handled by a `run_workbench_command` arm in `window_tab.rs`;
//! - `palette.rs` fills the palette for the kind, and `from_input` and
//!   `get_input` handle every kind without panicking.
//!
//! The source checks read the files at compile time and look for the kind's
//! name, so they are a safety net and not a proof of correct behaviour.
use std::collections::HashSet;

use strum::{EnumMessage, IntoEnumIterator};

use crate::{command::LapceWorkbenchCommand, palette::kind::PaletteKind};

const WINDOW_TAB: &str = include_str!("window_tab.rs");
const PALETTE: &str = include_str!("palette.rs");

/// Kinds that are reached through a typed prefix or from another command
/// rather than a command of their own.
const NO_COMMAND: &[&str] = &[
    "File", "FileChooser", "Line", "Command", "Workspace", "Reference", "DocumentSymbol",
    "WorkspaceSymbol", "SshHost", "RunAndDebug", "ColorTheme", "IconTheme", "Language",
    "LineEnding", "SCMReferences", "TerminalProfile", "DiffFiles", "HelpAndFile", "PaletteHelp",
    "ImportClass", "WrapTag", "RenameTag", "WslHost",
];

#[test]
fn every_palette_kind_is_wired() {
    let mut problems = Vec::new();
    for kind in PaletteKind::iter() {
        let name = format!("{kind:?}");
        match kind.command() {
            None => {
                if !NO_COMMAND.contains(&name.as_str()) {
                    problems.push(format!("{name}: no command() and not listed in NO_COMMAND"));
                }
            }
            Some(cmd) => {
                if cmd.get_message().map_or(true, |m| m.trim().is_empty()) {
                    problems.push(format!("{name}: command {cmd:?} has no display message"));
                }
                if !WINDOW_TAB.contains(&format!("{cmd:?} =>")) {
                    problems.push(format!("{name}: window_tab.rs has no arm for command {cmd:?}"));
                }
            }
        }
        if !PALETTE.contains(&format!("PaletteKind::{name}")) {
            problems.push(format!("{name}: palette.rs has no arm for this kind"));
        }
        // Must not panic for any kind.
        let _ = kind.symbol();
        let _ = kind.get_input("x");
    }
    assert!(problems.is_empty(), "palette wiring gaps:\n{}", problems.join("\n"));
}

#[test]
fn palette_commands_have_unique_names() {
    let mut seen = HashSet::new();
    for cmd in LapceWorkbenchCommand::iter() {
        let s: &'static str = (&cmd).into();
        assert!(seen.insert(s), "duplicate command name {s}");
    }
}

#[test]
fn kinds_with_a_command_round_trip_from_the_symbol() {
    // `from_input` of a kind's own symbol must give back a kind that handles
    // the input without panicking (no check of identity: several kinds share
    // the file palette).
    for kind in PaletteKind::iter() {
        let input = format!("{}abc", kind.symbol());
        let k = PaletteKind::from_input(&input);
        let _ = k.get_input(&input);
    }
}
