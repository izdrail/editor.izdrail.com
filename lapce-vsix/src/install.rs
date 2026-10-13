//! Writes an imported `.vsix` into the `vsx/` store.
//!
//! Layout (one directory per extension, `<publisher>.<name>`):
//! - `volt.toml`        data-only package metadata read by the existing theme loaders
//! - `themes/*.toml`    converted color themes
//! - `icon-themes/`     converted icon themes, icons under `icons/`
//! - `grammars/*.json`  TextMate grammars (verbatim)
//! - `snippets/*.json`  normalised snippets
//! - `langconfig/*.json` normalised language configuration
//! - `vsx.json`         manifest cache (what was imported, languages, licence, code flag)
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    icons, langconfig, snippets, textmate, theme,
    vsix::{Vsix, resolve_rel},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LanguageEntry {
    pub id: String,
    pub extensions: Vec<String>,
    pub filenames: Vec<String>,
    pub aliases: Vec<String>,
    pub config: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GrammarEntry {
    pub language: Option<String>,
    pub scope_name: String,
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnippetEntry {
    pub language: Option<String>,
    pub path: String,
}

/// What was imported; also written to `vsx.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportReport {
    pub id: String,
    pub version: String,
    pub display_name: String,
    pub license: Option<String>,
    pub license_file_present: bool,
    /// The extension declares `main`/`browser`. That code is not loaded.
    pub has_code: bool,
    pub code_entries: Vec<String>,
    pub color_themes: Vec<String>,
    pub icon_themes: Vec<String>,
    pub languages: Vec<LanguageEntry>,
    pub grammars: Vec<GrammarEntry>,
    pub snippets: Vec<SnippetEntry>,
    pub snippet_count: usize,
    pub icon_mappings: usize,
    /// Declared content that could not be imported, with the reason.
    pub skipped: Vec<String>,
    #[serde(skip)]
    pub dir: PathBuf,
}

fn slug(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    s.trim_matches('.').to_owned()
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}

fn strs(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Import a `.vsix` into `<vsx_root>/<publisher>.<name>`, replacing any
/// previous import of the same extension.
pub fn import_vsix(bytes: &[u8], vsx_root: &Path) -> Result<ImportReport> {
    let mut v = Vsix::open(bytes.to_vec())?;
    let m = v.manifest.clone();
    let id = slug(&m.id());
    let mut rep = ImportReport {
        id: id.clone(),
        version: m.version.clone(),
        display_name: m.display_name.clone(),
        license: m.license.clone(),
        license_file_present: v.license_text().is_some(),
        has_code: m.has_code(),
        code_entries: m.code_entries.clone(),
        ..Default::default()
    };

    fs::create_dir_all(vsx_root)?;
    let tmp = vsx_root.join(format!(".{id}.importing"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp)?;

    // ---- color themes
    let mut theme_files = Vec::new();
    for t in m.contributes("themes") {
        let label = t
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or(&m.display_name);
        let Some(path) = t.get("path").and_then(Value::as_str) else {
            continue;
        };
        let Some(rel) = crate::vsix::normalize_rel(path) else {
            rep.skipped.push(format!("theme {label}: bad path"));
            continue;
        };
        let doc = match v.read_json(&rel) {
            Ok(d) => d,
            Err(e) => {
                rep.skipped.push(format!("theme {label}: {e}"));
                continue;
            }
        };
        let mut includes: Vec<(String, Value)> = Vec::new();
        let rel2 = rel.clone();
        let parsed = {
            let mut loader = |inc: &str| -> Option<Value> {
                let r = resolve_rel(&rel2, inc)?;
                let d = v.read_json(&r).ok()?;
                includes.push((r, Value::Null));
                Some(d)
            };
            theme::parse_theme(&doc, &mut loader, 0)
        };
        match parsed {
            Ok(mut parsed) => {
                if parsed.kind.is_empty() {
                    parsed.kind = match t.get("uiTheme").and_then(Value::as_str) {
                        Some("vs") => "light".into(),
                        Some("hc-black") => "hc-dark".into(),
                        Some("hc-light") => "hc-light".into(),
                        _ => "dark".into(),
                    };
                }
                let toml_text = theme::to_lapce_toml(label, &parsed);
                let file = format!("themes/{}.toml", slug(label));
                write(&tmp.join(&file), toml_text.as_bytes())?;
                theme_files.push(file);
                rep.color_themes.push(label.to_owned());
            }
            Err(e) => rep.skipped.push(format!("theme {label}: {e}")),
        }
    }

    // ---- icon themes
    let mut icon_files = Vec::new();
    for t in m.contributes("iconThemes") {
        let label = t
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or(&m.display_name);
        let Some(path) = t.get("path").and_then(Value::as_str) else {
            continue;
        };
        let Some(rel) = crate::vsix::normalize_rel(path) else {
            continue;
        };
        let doc = match v.read_json(&rel) {
            Ok(d) => d,
            Err(e) => {
                rep.skipped.push(format!("icon theme {label}: {e}"));
                continue;
            }
        };
        let conv = icons::convert(label, &rel, &doc);
        let dir = format!("icon-themes/{}", slug(label));
        for (src, flat) in &conv.copy {
            match v.read(src) {
                Ok(b) => write(&tmp.join(&dir).join(flat), &b)?,
                Err(e) => rep.skipped.push(format!("icon {src}: {e}")),
            }
        }
        write(
            &tmp.join(&dir).join("icon-theme.toml"),
            conv.toml.as_bytes(),
        )?;
        icon_files.push(format!("{dir}/icon-theme.toml"));
        rep.icon_themes.push(label.to_owned());
        rep.icon_mappings += conv.extensions + conv.filenames + conv.folders;
        for s in conv.skipped.iter().take(3) {
            rep.skipped.push(format!("icon theme {label}: {s}"));
        }
        if conv.skipped.len() > 3 {
            rep.skipped.push(format!(
                "icon theme {label}: {} more unsupported mappings",
                conv.skipped.len() - 3
            ));
        }
    }

    // ---- languages (+ configuration)
    for l in m.contributes("languages") {
        let Some(lid) = l.get("id").and_then(Value::as_str) else {
            continue;
        };
        let mut entry = LanguageEntry {
            id: lid.to_owned(),
            extensions: strs(l.get("extensions")),
            filenames: strs(l.get("filenames")),
            aliases: strs(l.get("aliases")),
            config: None,
        };
        if let Some(cfg) = l.get("configuration").and_then(Value::as_str) {
            match v.read_json(cfg) {
                Ok(doc) => {
                    let parsed = langconfig::parse(&doc);
                    let file = format!("langconfig/{}.json", slug(lid));
                    write(
                        &tmp.join(&file),
                        serde_json::to_string_pretty(&parsed)?.as_bytes(),
                    )?;
                    entry.config = Some(file);
                }
                Err(e) => rep
                    .skipped
                    .push(format!("language {lid} configuration: {e}")),
            }
        }
        rep.languages.push(entry);
    }

    // ---- grammars
    for g in m.contributes("grammars") {
        let (Some(scope), Some(path)) = (
            g.get("scopeName").and_then(Value::as_str),
            g.get("path").and_then(Value::as_str),
        ) else {
            continue;
        };
        if path.ends_with(".tmLanguage") || path.ends_with(".plist") {
            rep.skipped
                .push(format!("grammar {scope}: plist grammars are not supported"));
            continue;
        }
        let Some(rel) = crate::vsix::normalize_rel(path) else {
            continue;
        };
        match v.read_json(&rel) {
            Ok(doc) => {
                if let Err(e) = textmate::Grammar::parse(&doc) {
                    rep.skipped.push(format!("grammar {scope}: {e}"));
                    continue;
                }
                let file = format!("grammars/{}.json", slug(scope));
                write(&tmp.join(&file), serde_json::to_string(&doc)?.as_bytes())?;
                rep.grammars.push(GrammarEntry {
                    language: g
                        .get("language")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    scope_name: scope.to_owned(),
                    path: file,
                });
            }
            Err(e) => rep.skipped.push(format!("grammar {scope}: {e}")),
        }
    }

    // ---- snippets
    for (i, s) in m.contributes("snippets").iter().enumerate() {
        let Some(path) = s.get("path").and_then(Value::as_str) else {
            continue;
        };
        let lang = s.get("language").and_then(Value::as_str).map(str::to_owned);
        match v.read_json(path) {
            Ok(doc) => {
                let list = snippets::parse(&doc);
                rep.snippet_count += list.len();
                let file = format!(
                    "snippets/{}-{i}.json",
                    slug(lang.as_deref().unwrap_or("global"))
                );
                let body = serde_json::json!({ "language": lang, "snippets": list });
                write(&tmp.join(&file), serde_json::to_string(&body)?.as_bytes())?;
                rep.snippets.push(SnippetEntry {
                    language: lang,
                    path: file,
                });
            }
            Err(e) => rep.skipped.push(format!("snippets {path}: {e}")),
        }
    }

    // ---- not importable
    for (key, what) in [
        ("commands", "commands"),
        ("debuggers", "debuggers"),
        ("views", "views"),
        ("viewsContainers", "view containers"),
        ("configuration", "settings"),
        ("keybindings", "keybindings"),
        ("productIconThemes", "product icon themes"),
        ("taskDefinitions", "task definitions"),
        ("menus", "menus"),
    ] {
        if m.raw.get("contributes").and_then(|c| c.get(key)).is_some() {
            rep.skipped
                .push(format!("{what}: needs extension code, not imported"));
        }
    }
    if rep.has_code {
        rep.skipped.push(format!(
            "contains code, not loaded ({})",
            rep.code_entries.join(", ")
        ));
    }

    // ---- metadata
    let mut volt = toml::Table::new();
    let s = |x: &str| toml::Value::String(x.to_owned());
    volt.insert("name".into(), s(&id));
    volt.insert("version".into(), s(&m.version));
    volt.insert("display-name".into(), s(&m.display_name));
    volt.insert("author".into(), s(&m.publisher));
    volt.insert("description".into(), s(&m.description));
    if let Some(r) = &m.repository {
        volt.insert("repository".into(), s(r));
    }
    if !theme_files.is_empty() {
        volt.insert(
            "color-themes".into(),
            toml::Value::Array(theme_files.iter().map(|f| s(f)).collect()),
        );
    }
    if !icon_files.is_empty() {
        volt.insert(
            "icon-themes".into(),
            toml::Value::Array(icon_files.iter().map(|f| s(f)).collect()),
        );
    }
    write(&tmp.join("volt.toml"), toml::to_string(&volt)?.as_bytes())?;
    write(
        &tmp.join("vsx.json"),
        serde_json::to_string_pretty(&rep)?.as_bytes(),
    )?;

    let dest = vsx_root.join(&id);
    let _ = fs::remove_dir_all(&dest);
    fs::rename(&tmp, &dest).context("install directory")?;
    rep.dir = dest;
    Ok(rep)
}

/// Where imported content lives: `<vsx_root>/*` directories that contain a
/// `vsx.json`.
pub fn installed_dirs(vsx_root: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(vsx_root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(".")
                    .starts_with('.')
                && p.join("vsx.json").is_file()
        })
        .collect();
    v.sort();
    v
}

/// Grammar registry plus file -> grammar association built from every
/// imported extension.
#[derive(Default)]
pub struct Loaded {
    pub registry: textmate::Registry,
    by_ext: BTreeMap<String, String>,
    by_name: BTreeMap<String, String>,
    pub snippets: Vec<(Option<String>, Vec<snippets::Snippet>)>,
    pub language_configs: BTreeMap<String, langconfig::LanguageConfig>,
}

impl Loaded {
    pub fn scope_for_path(&self, path: &Path) -> Option<&str> {
        let name = path.file_name()?.to_str()?;
        if let Some(s) = self.by_name.get(name) {
            return Some(s);
        }
        let lower = name.to_ascii_lowercase();
        // longest matching suffix first: "d.ts" before "ts"
        let mut rest = lower.as_str();
        while let Some((_, tail)) = rest.split_once('.') {
            if let Some(s) = self.by_ext.get(tail) {
                return Some(s);
            }
            rest = tail;
        }
        None
    }

    pub fn snippets_for(&self, language: &str) -> Vec<&snippets::Snippet> {
        self.snippets
            .iter()
            .filter(|(l, _)| l.as_deref() == Some(language) || l.is_none())
            .flat_map(|(l, v)| {
                v.iter().filter(move |s| {
                    l.is_some()
                        || s.scope.is_empty()
                        || s.scope.iter().any(|x| x == language)
                })
            })
            .collect()
    }
}

pub fn load_registry(vsx_root: &Path) -> Arc<Loaded> {
    let mut out = Loaded::default();
    let mut lang_scope: BTreeMap<String, String> = BTreeMap::new();
    let mut pending: Vec<(PathBuf, ImportReport)> = Vec::new();
    for dir in installed_dirs(vsx_root) {
        let Ok(text) = fs::read_to_string(dir.join("vsx.json")) else {
            continue;
        };
        let Ok(rep) = serde_json::from_str::<ImportReport>(&text) else {
            continue;
        };
        for g in &rep.grammars {
            let Ok(t) = fs::read_to_string(dir.join(&g.path)) else {
                continue;
            };
            let Ok(doc) = serde_json::from_str::<Value>(&t) else {
                continue;
            };
            if out.registry.add_json(&doc).is_ok() {
                if let Some(l) = &g.language {
                    lang_scope.insert(l.clone(), g.scope_name.clone());
                }
            }
        }
        for s in &rep.snippets {
            let Ok(t) = fs::read_to_string(dir.join(&s.path)) else {
                continue;
            };
            if let Ok(v) = serde_json::from_str::<Value>(&t) {
                if let Ok(list) = serde_json::from_value::<Vec<snippets::Snippet>>(
                    v.get("snippets").cloned().unwrap_or_default(),
                ) {
                    out.snippets.push((s.language.clone(), list));
                }
            }
        }
        pending.push((dir, rep));
    }
    for (dir, rep) in pending {
        for l in &rep.languages {
            if let Some(cfg) = &l.config {
                if let Ok(t) = fs::read_to_string(dir.join(cfg)) {
                    if let Ok(c) = serde_json::from_str(&t) {
                        out.language_configs.insert(l.id.clone(), c);
                    }
                }
            }
            let Some(scope) = lang_scope.get(&l.id) else {
                continue;
            };
            for e in &l.extensions {
                out.by_ext.insert(
                    e.trim_start_matches('.').to_ascii_lowercase(),
                    scope.clone(),
                );
            }
            for f in &l.filenames {
                out.by_name.insert(f.clone(), scope.clone());
            }
        }
    }
    Arc::new(out)
}
