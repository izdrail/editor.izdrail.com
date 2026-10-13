//! VS Code color theme JSON -> Lapce color-theme TOML.
use std::collections::BTreeMap;

use anyhow::{Result, bail};
use serde_json::Value;

/// A resolved VS Code theme: workbench colors plus token rules.
#[derive(Debug, Default, Clone)]
pub struct VsTheme {
    pub kind: String,
    pub colors: BTreeMap<String, String>,
    /// (selector, foreground)
    pub rules: Vec<(String, String)>,
}

/// Parse a theme document. `load_include` fetches an included theme file
/// (path relative to the including file).
pub fn parse_theme(
    doc: &Value,
    load_include: &mut dyn FnMut(&str) -> Option<Value>,
    depth: usize,
) -> Result<VsTheme> {
    let mut theme = VsTheme::default();
    if depth < 4 {
        if let Some(inc) = doc.get("include").and_then(Value::as_str) {
            if let Some(parent) = load_include(inc) {
                theme = parse_theme(&parent, load_include, depth + 1)?;
            }
        }
    }
    if let Some(t) = doc.get("type").and_then(Value::as_str) {
        theme.kind = t.to_lowercase();
    }
    if let Some(colors) = doc.get("colors").and_then(Value::as_object) {
        for (k, v) in colors {
            if let Some(c) = v.as_str() {
                theme.colors.insert(k.clone(), c.to_owned());
            }
        }
    }
    match doc.get("tokenColors") {
        Some(Value::Array(rules)) => {
            for r in rules {
                let Some(fg) = r
                    .get("settings")
                    .and_then(|s| s.get("foreground"))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                let scopes: Vec<String> = match r.get("scope") {
                    Some(Value::String(s)) => {
                        s.split(',').map(|s| s.trim().to_owned()).collect()
                    }
                    Some(Value::Array(a)) => a
                        .iter()
                        .filter_map(Value::as_str)
                        .flat_map(|s| s.split(','))
                        .map(|s| s.trim().to_owned())
                        .collect(),
                    // A rule without scope sets the default foreground.
                    None => vec![String::new()],
                    _ => continue,
                };
                for s in scopes {
                    theme.rules.push((s, fg.to_owned()));
                }
            }
        }
        Some(Value::String(_)) => {
            bail!("tokenColors references an external tmTheme file (unsupported)")
        }
        _ => {}
    }
    // The newer `settings` form used by some legacy themes is not handled.
    Ok(theme)
}

fn scope_matches(selector: &str, scope: &str) -> bool {
    scope == selector
        || (scope.len() > selector.len()
            && scope.starts_with(selector)
            && scope.as_bytes()[selector.len()] == b'.')
}

impl VsTheme {
    /// Foreground the theme gives to a TextMate scope: the longest matching
    /// single-atom selector wins, later rules win ties.
    pub fn color_for_scope(&self, scope: &str) -> Option<&str> {
        let mut best: Option<(usize, &str)> = None;
        for (sel, fg) in &self.rules {
            if sel.is_empty() || sel.contains(' ') {
                continue; // descendant / exclusion selectors are not supported
            }
            if scope_matches(sel, scope)
                && best.map(|(l, _)| sel.len() >= l).unwrap_or(true)
            {
                best = Some((sel.len(), fg));
            }
        }
        best.map(|(_, c)| c)
    }

    pub fn default_foreground(&self) -> Option<&str> {
        self.colors
            .get("editor.foreground")
            .map(String::as_str)
            .or_else(|| {
                self.rules
                    .iter()
                    .find(|(s, _)| s.is_empty())
                    .map(|(_, c)| c.as_str())
            })
    }
}

/// Lapce syntax key and the TextMate scope that best represents it.
const SYNTAX_KEYS: &[(&str, &str)] = &[
    ("comment", "comment"),
    ("constant", "constant.language"),
    ("type", "entity.name.type"),
    ("typeAlias", "entity.name.type"),
    ("number", "constant.numeric"),
    ("enum", "entity.name.type.enum"),
    ("struct", "entity.name.type.struct"),
    ("structure", "entity.name.type.struct"),
    ("interface", "entity.name.type.interface"),
    ("attribute", "entity.other.attribute-name"),
    ("constructor", "entity.name.function.constructor"),
    ("function", "entity.name.function"),
    ("method", "entity.name.function"),
    ("function.method", "entity.name.function"),
    ("keyword", "keyword"),
    ("selfKeyword", "variable.language"),
    ("field", "variable.other.property"),
    ("property", "variable.other.property"),
    ("enumMember", "variable.other.enummember"),
    ("enum-member", "variable.other.enummember"),
    ("string", "string"),
    ("type.builtin", "support.type"),
    ("builtinType", "support.type"),
    ("escape", "constant.character.escape"),
    ("string.escape", "constant.character.escape"),
    ("embedded", "meta.embedded"),
    ("punctuation.delimiter", "punctuation"),
    ("text.title", "markup.heading"),
    ("text.uri", "markup.underline.link"),
    ("text.reference", "markup.underline.link"),
    ("variable", "variable"),
    ("variable.other.member", "variable.other.property"),
    ("tag", "entity.name.tag"),
    ("markup.heading", "markup.heading"),
    ("markup.bold", "markup.bold"),
    ("markup.italic", "markup.italic"),
    ("markup.list", "markup.list"),
    ("markup.link.url", "markup.underline.link"),
    ("markup.link.label", "string.other.link"),
    ("markup.link.text", "string.other.link"),
];

/// VS Code workbench color id -> Lapce `ui` key.
const UI_MAP: &[(&str, &str)] = &[
    ("editor.background", "editor.background"),
    ("editor.foreground", "editor.foreground"),
    ("editorCursor.foreground", "editor.caret"),
    ("editor.selectionBackground", "editor.selection"),
    ("editor.lineHighlightBackground", "editor.current_line"),
    ("editorIndentGuide.background", "editor.indent_guide"),
    ("editorIndentGuide.background1", "editor.indent_guide"),
    ("editorWhitespace.foreground", "editor.visible_whitespace"),
    ("editorLink.activeForeground", "editor.link"),
    ("editorLineNumber.foreground", "editor.dim"),
    ("editorLineNumber.activeForeground", "editor.focus"),
    (
        "editorStickyScroll.background",
        "editor.sticky_header_background",
    ),
    ("tab.activeBackground", "lapce.tab.active.background"),
    ("tab.activeForeground", "lapce.tab.active.foreground"),
    ("tab.activeBorderTop", "lapce.tab.active.underline"),
    ("tab.inactiveBackground", "lapce.tab.inactive.background"),
    ("tab.inactiveForeground", "lapce.tab.inactive.foreground"),
    ("panel.border", "lapce.border"),
    ("scrollbarSlider.background", "lapce.scroll_bar"),
    ("errorForeground", "lapce.error"),
    ("editorWarning.foreground", "lapce.warn"),
    ("button.background", "lapce.button.primary.background"),
    ("button.foreground", "lapce.button.primary.foreground"),
    ("quickInput.background", "palette.background"),
    ("quickInput.foreground", "palette.foreground"),
    (
        "list.activeSelectionBackground",
        "palette.current.background",
    ),
    (
        "list.activeSelectionForeground",
        "palette.current.foreground",
    ),
    ("editorSuggestWidget.background", "completion.background"),
    (
        "editorSuggestWidget.selectedBackground",
        "completion.current",
    ),
    ("editorHoverWidget.background", "hover.background"),
    ("editorHoverWidget.foreground", "tooltip.foreground"),
    ("editorHoverWidget.background", "tooltip.background"),
    ("editorInlayHint.foreground", "inlay_hint.foreground"),
    ("editorInlayHint.background", "inlay_hint.background"),
    ("editorGutter.addedBackground", "source_control.added"),
    ("editorGutter.deletedBackground", "source_control.removed"),
    ("editorGutter.modifiedBackground", "source_control.modified"),
    ("activityBar.background", "activity.background"),
    ("sideBar.background", "panel.background"),
    ("sideBar.foreground", "panel.foreground"),
    ("list.hoverBackground", "panel.hovered.background"),
    (
        "list.inactiveSelectionBackground",
        "panel.current.background",
    ),
    ("statusBar.background", "status.background"),
    ("statusBar.foreground", "status.foreground"),
    ("terminal.foreground", "terminal.foreground"),
    ("terminal.background", "terminal.background"),
    ("terminalCursor.foreground", "terminal.cursor"),
    ("terminal.ansiBlack", "terminal.black"),
    ("terminal.ansiRed", "terminal.red"),
    ("terminal.ansiGreen", "terminal.green"),
    ("terminal.ansiYellow", "terminal.yellow"),
    ("terminal.ansiBlue", "terminal.blue"),
    ("terminal.ansiMagenta", "terminal.magenta"),
    ("terminal.ansiCyan", "terminal.cyan"),
    ("terminal.ansiWhite", "terminal.white"),
    ("terminal.ansiBrightBlack", "terminal.bright_black"),
    ("terminal.ansiBrightRed", "terminal.bright_red"),
    ("terminal.ansiBrightGreen", "terminal.bright_green"),
    ("terminal.ansiBrightYellow", "terminal.bright_yellow"),
    ("terminal.ansiBrightBlue", "terminal.bright_blue"),
    ("terminal.ansiBrightMagenta", "terminal.bright_magenta"),
    ("terminal.ansiBrightCyan", "terminal.bright_cyan"),
    ("terminal.ansiBrightWhite", "terminal.bright_white"),
];

/// Base palette entries Lapce theme variables are built from.
const BASE_MAP: &[(&str, &str)] = &[
    ("black", "terminal.ansiBlack"),
    ("red", "terminal.ansiRed"),
    ("green", "terminal.ansiGreen"),
    ("yellow", "terminal.ansiYellow"),
    ("blue", "terminal.ansiBlue"),
    ("magenta", "terminal.ansiMagenta"),
    ("purple", "terminal.ansiMagenta"),
    ("cyan", "terminal.ansiCyan"),
    ("white", "terminal.ansiWhite"),
];

fn valid_color(c: &str) -> bool {
    let c = c.trim();
    c.starts_with('#')
        && matches!(c.len(), 4 | 5 | 7 | 9)
        && c[1..].chars().all(|ch| ch.is_ascii_hexdigit())
}

/// Convert a resolved VS Code theme into Lapce color-theme TOML text.
pub fn to_lapce_toml(name: &str, theme: &VsTheme) -> String {
    let light = theme.kind == "light" || theme.kind == "hc-light";
    let get = |k: &str| theme.colors.get(k).filter(|c| valid_color(c)).cloned();
    let fg = get("editor.foreground")
        .or_else(|| {
            theme
                .default_foreground()
                .filter(|c| valid_color(c))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| {
            if light {
                "#333333".into()
            } else {
                "#D4D4D4".into()
            }
        });
    let bg = get("editor.background").unwrap_or_else(|| {
        if light {
            "#FFFFFF".into()
        } else {
            "#1E1E1E".into()
        }
    });

    let mut base = toml::Table::new();
    let put = |t: &mut toml::Table, k: &str, v: String| {
        t.insert(k.to_owned(), toml::Value::String(v));
    };
    for (key, vs) in BASE_MAP {
        if let Some(c) = get(vs) {
            put(&mut base, key, c);
        }
    }
    put(&mut base, "primary-background", bg.clone());
    put(
        &mut base,
        "secondary-background",
        get("sideBar.background")
            .or_else(|| get("editorWidget.background"))
            .unwrap_or_else(|| bg.clone()),
    );
    put(
        &mut base,
        "current-background",
        get("editor.lineHighlightBackground")
            .or_else(|| get("list.activeSelectionBackground"))
            .unwrap_or_else(|| bg.clone()),
    );
    put(&mut base, "text", fg.clone());
    let comment = theme
        .color_for_scope("comment")
        .filter(|c| valid_color(c))
        .map(str::to_owned);
    put(
        &mut base,
        "dim-text",
        get("editorLineNumber.foreground")
            .or(comment)
            .unwrap_or_else(|| fg.clone()),
    );

    let mut syntax = toml::Table::new();
    // Every scope the theme names is emitted verbatim so TextMate-highlighted
    // text resolves with the same prefix rules VS Code uses.
    for (sel, color) in &theme.rules {
        if sel.is_empty() || sel.contains(' ') || !valid_color(color) {
            continue;
        }
        put(&mut syntax, sel, color.clone());
    }
    // Keys the Lexilla highlighter emits (and the default theme defines):
    // always set them so no default-theme colour leaks into this theme.
    for (key, scope) in SYNTAX_KEYS {
        if syntax.contains_key(*key) && !key.contains('.') {
            // A scope of the same name already carries the theme's colour.
            continue;
        }
        let color = theme
            .color_for_scope(scope)
            .filter(|c| valid_color(c))
            .map(str::to_owned)
            .unwrap_or_else(|| fg.clone());
        put(&mut syntax, key, color);
    }
    if let Some(c) = theme.color_for_scope("markup.deleted") {
        put(&mut syntax, "markup.deleted", c.to_owned());
    }

    let mut ui = toml::Table::new();
    for (vs, lapce) in UI_MAP {
        if let Some(c) = get(vs) {
            put(&mut ui, lapce, c);
        }
    }

    let mut head = toml::Table::new();
    put(&mut head, "name", name.to_owned());
    if theme.kind.starts_with("hc") {
        head.insert("high-contrast".into(), toml::Value::Boolean(true));
    }
    head.insert("base".into(), toml::Value::Table(base));
    head.insert("syntax".into(), toml::Value::Table(syntax));
    head.insert("ui".into(), toml::Value::Table(ui));
    let mut root = toml::Table::new();
    root.insert("color-theme".into(), toml::Value::Table(head));
    toml::to_string(&root).expect("theme serialises")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_basic_theme() {
        let doc = crate::jsonc::parse(
            r##"{"type":"dark","colors":{"editor.background":"#282a36","editor.foreground":"#f8f8f2","terminal.ansiRed":"#ff5555"},
            "tokenColors":[{"scope":["comment","punctuation.definition.comment"],"settings":{"foreground":"#6272a4"}},
            {"scope":"keyword.control, storage.type","settings":{"foreground":"#ff79c6"}},
            {"scope":"string","settings":{"foreground":"#f1fa8c"}}]}"##,
        )
        .unwrap();
        let t = parse_theme(&doc, &mut |_| None, 0).unwrap();
        let toml_text = to_lapce_toml("Test", &t);
        let v: toml::Value = toml::from_str(&toml_text).unwrap();
        let ct = &v["color-theme"];
        assert_eq!(ct["name"].as_str(), Some("Test"));
        assert_eq!(ct["base"]["primary-background"].as_str(), Some("#282a36"));
        assert_eq!(ct["base"]["red"].as_str(), Some("#ff5555"));
        assert_eq!(ct["syntax"]["comment"].as_str(), Some("#6272a4"));
        assert_eq!(ct["syntax"]["keyword.control"].as_str(), Some("#ff79c6"));
        assert_eq!(ct["syntax"]["string"].as_str(), Some("#f1fa8c"));
        // not named by the theme -> falls back to the editor foreground
        assert_eq!(ct["syntax"]["number"].as_str(), Some("#f8f8f2"));
        assert_eq!(ct["ui"]["editor.background"].as_str(), Some("#282a36"));
    }

    #[test]
    fn longest_selector_wins() {
        let mut t = VsTheme::default();
        t.rules.push(("string".into(), "#111111".into()));
        t.rules.push(("string.quoted".into(), "#222222".into()));
        assert_eq!(t.color_for_scope("string.quoted.double"), Some("#222222"));
        assert_eq!(t.color_for_scope("string.regexp"), Some("#111111"));
        assert_eq!(t.color_for_scope("strings"), None);
    }
}
