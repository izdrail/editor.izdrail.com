//! TextMate-grammar highlighting from imported VS Code extensions, used for
//! files Lexilla has no lexer for.
use std::{
    path::Path,
    sync::{Arc, RwLock},
};

use lapce_rpc::style::{LineStyle, Style};
use lapce_vsix::{install::Loaded, textmate::Snapshot};
use lapce_xi_rope::Rope;

use crate::directory::Directory;

static LOADED: RwLock<Option<Arc<Loaded>>> = RwLock::new(None);

/// Imported content, loaded from the `vsx/` directory on first use.
pub fn loaded() -> Arc<Loaded> {
    if let Some(l) = LOADED.read().unwrap().as_ref() {
        return l.clone();
    }
    let l = match Directory::vsx_directory() {
        Some(dir) => lapce_vsix::load_registry(&dir),
        None => Arc::new(Loaded::default()),
    };
    *LOADED.write().unwrap() = Some(l.clone());
    l
}

/// Pick up newly imported extensions.
pub fn reload() {
    *LOADED.write().unwrap() = None;
}

/// Grammar scope for a file, if an imported extension provides one.
pub fn scope_for_path(path: &Path) -> Option<String> {
    loaded().scope_for_path(path).map(str::to_owned)
}

pub fn highlight(scope: &str, text: &Rope) -> Option<Arc<Snapshot>> {
    loaded()
        .registry
        .highlight(scope, &text.to_string())
        .map(Arc::new)
}

pub fn line_styles(snapshot: &Snapshot, text: &Rope, line: usize) -> Vec<LineStyle> {
    if snapshot.len != text.len() {
        return Vec::new();
    }
    snapshot
        .lines
        .get(line)
        .map(|runs| {
            runs.iter()
                .filter(|(_, _, key)| !key.is_empty())
                .map(|(s, e, key)| LineStyle {
                    start: *s as usize,
                    end: *e as usize,
                    style: Style {
                        fg_color: Some(key.to_string()),
                    },
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Load imported extensions from `root` instead of the default `vsx/` directory.
pub fn use_root(root: &Path) {
    *LOADED.write().unwrap() = Some(lapce_vsix::load_registry(root));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::Syntax;

    #[test]
    #[ignore = "needs LAPCE_VSX_TEST_ROOT with vscode.docker imported"]
    fn imported_grammar_highlights_through_syntax() {
        let root = std::env::var("LAPCE_VSX_TEST_ROOT").unwrap();
        use_root(Path::new(&root));
        let mut syntax = Syntax::init(Path::new("Dockerfile"));
        assert!(syntax.textmate_scope.is_some());
        let text = Rope::from("FROM alpine\n# note\nRUN echo \"hi\"\n");
        syntax.parse(1, text.clone(), None);
        let snap = syntax.textmate.clone().expect("textmate snapshot");
        let l0 = line_styles(&snap, &text, 0);
        assert!(
            l0.iter().any(|s| s.start == 0
                && s.end == 4
                && s.style.fg_color.as_deref().unwrap().contains("keyword")),
            "{l0:?}"
        );
        let l1 = line_styles(&snap, &text, 1);
        assert!(l1[0].style.fg_color.as_deref().unwrap().contains("comment"));
        let l2 = line_styles(&snap, &text, 2);
        assert!(
            l2.iter().any(|s| s
                .style
                .fg_color
                .as_deref()
                .unwrap()
                .contains("string")),
            "{l2:?}"
        );
    }
}
