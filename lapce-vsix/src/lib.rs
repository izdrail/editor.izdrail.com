//! Import of the declarative parts of VS Code extensions (`.vsix`): color
//! themes, file icon themes, snippets, language configuration and TextMate
//! grammars. Extension code (`main` / `browser`) is never loaded or run.
pub mod icons;
pub mod install;
pub mod jsonc;
pub mod langconfig;
pub mod manifest;
pub mod snippets;
pub mod textmate;
pub mod theme;
pub mod vsix;

pub use install::{ImportReport, import_vsix, load_registry};
