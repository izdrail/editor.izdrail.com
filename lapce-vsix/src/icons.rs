//! VS Code file icon theme JSON -> Lapce icon-theme TOML (SVG icons only).
use std::collections::BTreeMap;

use serde_json::Value;

use crate::vsix::resolve_rel;

#[derive(Debug, Default)]
pub struct IconConversion {
    /// (root-relative source path, flat output file name)
    pub copy: Vec<(String, String)>,
    pub toml: String,
    pub extensions: usize,
    pub filenames: usize,
    pub folders: usize,
    /// Mappings dropped because they use unsupported features.
    pub skipped: Vec<String>,
}

fn flat_name(rel: &str) -> String {
    rel.replace('/', "__")
}

pub fn convert(name: &str, doc_path: &str, doc: &Value) -> IconConversion {
    let mut out = IconConversion::default();
    let defs = doc.get("iconDefinitions").and_then(Value::as_object);
    let mut copied: BTreeMap<String, String> = BTreeMap::new();
    let mut resolve = |id: &str, out: &mut IconConversion| -> Option<String> {
        let def = defs?.get(id)?;
        let Some(path) = def.get("iconPath").and_then(Value::as_str) else {
            out.skipped.push(format!("{id}: font icon"));
            return None;
        };
        if !path.to_ascii_lowercase().ends_with(".svg") {
            out.skipped.push(format!("{id}: not svg"));
            return None;
        }
        let rel = resolve_rel(doc_path, path)?;
        let flat = copied
            .entry(rel.clone())
            .or_insert_with(|| flat_name(&rel))
            .clone();
        if !out.copy.iter().any(|(r, _)| *r == rel) {
            out.copy.push((rel, flat.clone()));
        }
        Some(flat)
    };

    let mut table = |key: &str, out: &mut IconConversion| -> toml::Table {
        let mut t = toml::Table::new();
        if let Some(m) = doc.get(key).and_then(Value::as_object) {
            for (k, v) in m {
                let Some(id) = v.as_str() else { continue };
                // Lapce keys extensions by the last extension only.
                if key == "fileExtensions" && k.contains('.') {
                    out.skipped.push(format!("multi-dot extension {k}"));
                    continue;
                }
                if let Some(file) = resolve(id, out) {
                    t.insert(k.clone(), toml::Value::String(file));
                }
            }
        }
        t
    };
    let ext = table("fileExtensions", &mut out);
    let files = table("fileNames", &mut out);
    let folders = table("folderNames", &mut out);
    out.extensions = ext.len();
    out.filenames = files.len();
    out.folders = folders.len();

    let mut head = toml::Table::new();
    head.insert("name".into(), toml::Value::String(name.to_owned()));
    head.insert("foldername".into(), toml::Value::Table(folders));
    head.insert("filename".into(), toml::Value::Table(files));
    head.insert("extension".into(), toml::Value::Table(ext));
    let mut root = toml::Table::new();
    root.insert("icon-theme".into(), toml::Value::Table(head));
    out.toml = toml::to_string(&root).expect("icon theme serialises");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_extensions_and_skips_non_svg() {
        let doc = crate::jsonc::parse(
            r#"{"iconDefinitions":{"a":{"iconPath":"./icons/a.svg"},"b":{"iconPath":"./icons/b.png"}},
            "fileExtensions":{"rs":"a","png":"b","min.js":"a"},"fileNames":{"Cargo.toml":"a"},"folderNames":{"src":"a"}}"#,
        )
        .unwrap();
        let c = convert("T", "dist/theme.json", &doc);
        assert_eq!(c.extensions, 1);
        assert_eq!(c.filenames, 1);
        assert_eq!(c.folders, 1);
        assert_eq!(c.copy.len(), 1);
        assert_eq!(c.copy[0].0, "dist/icons/a.svg");
        assert_eq!(c.skipped.len(), 2);
    }
}
