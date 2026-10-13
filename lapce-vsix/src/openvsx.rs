//! Open VSX registry client (https://open-vsx.org): search, download,
//! install, uninstall and list. Only Open VSX is used, never the Microsoft
//! Marketplace. Installing goes through [`crate::import_vsix`], so only
//! declarative content is imported and extension code is never run.
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{ImportReport, import_vsix, install::installed_dirs};

pub const DEFAULT_REGISTRY: &str = "https://open-vsx.org";
/// Refuse downloads larger than this.
pub const MAX_VSIX_BYTES: u64 = 100 * 1024 * 1024;

/// One search hit or extension detail.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Extension {
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub download_count: u64,
    pub average_rating: Option<f64>,
    pub deprecated: bool,
    pub files: Files,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Files {
    pub download: Option<String>,
    pub sha256: Option<String>,
    pub icon: Option<String>,
}

impl Extension {
    pub fn id(&self) -> String {
        format!("{}.{}", self.namespace, self.name)
    }
    pub fn title(&self) -> &str {
        self.display_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchResults {
    pub total: u64,
    pub offset: u64,
    pub extensions: Vec<Extension>,
}

/// Row of the installed list, read from each extension's `vsx.json`.
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub id: String,
    pub version: String,
    pub display_name: String,
    pub license: Option<String>,
    pub has_code: bool,
    pub color_themes: usize,
    pub icon_themes: usize,
    pub grammars: usize,
    pub snippets: usize,
}

pub struct Client {
    http: reqwest::blocking::Client,
    base: String,
}

impl Client {
    pub fn new(base: &str) -> Result<Self> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("lapce-editor-izdrail")
            .build()?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
        })
    }

    pub fn open_vsx() -> Result<Self> {
        Self::new(DEFAULT_REGISTRY)
    }

    pub fn search(
        &self,
        query: &str,
        size: u32,
        offset: u32,
    ) -> Result<SearchResults> {
        let url = reqwest::Url::parse_with_params(
            &format!("{}/api/-/search", self.base),
            &[
                ("query", query),
                ("size", &size.min(50).to_string()),
                ("offset", &offset.to_string()),
                ("sortBy", "relevance"),
                ("sortOrder", "desc"),
            ],
        )?;
        let body = self.get_text(url.as_str())?;
        parse_search(&body)
    }

    /// Latest version of `namespace.name`.
    pub fn detail(&self, id: &str) -> Result<Extension> {
        let (ns, name) = split_id(id)?;
        let body = self.get_text(&format!("{}/api/{ns}/{name}", self.base))?;
        parse_detail(&body)
    }

    fn get_text(&self, url: &str) -> Result<String> {
        let r = self
            .http
            .get(url)
            .send()
            .with_context(|| format!("GET {url}"))?;
        if !r.status().is_success() {
            bail!("GET {url}: HTTP {}", r.status());
        }
        Ok(r.text()?)
    }

    fn get_bytes(&self, url: &str, limit: u64) -> Result<Vec<u8>> {
        let r = self
            .http
            .get(url)
            .send()
            .with_context(|| format!("GET {url}"))?;
        if !r.status().is_success() {
            bail!("GET {url}: HTTP {}", r.status());
        }
        if r.content_length().is_some_and(|l| l > limit) {
            bail!("download larger than {limit} bytes, refusing");
        }
        let bytes = r.bytes()?;
        if bytes.len() as u64 > limit {
            bail!("download larger than {limit} bytes, refusing");
        }
        Ok(bytes.to_vec())
    }

    /// Download the latest version of `id`, verify it against the registry's
    /// sha256 when one is published, and import it into `vsx_root`.
    pub fn install(&self, id: &str, vsx_root: &Path) -> Result<ImportReport> {
        let ext = self.detail(id)?;
        if !ext
            .files
            .download
            .as_deref()
            .is_some_and(|u| self.trusted(u))
        {
            bail!("{id}: registry gave no download URL on {}", self.base);
        }
        let url = ext.files.download.as_deref().unwrap_or_default();
        let bytes = self.get_bytes(url, MAX_VSIX_BYTES)?;
        if let Some(sha_url) =
            ext.files.sha256.as_deref().filter(|u| self.trusted(u))
        {
            let expected = self.get_text(sha_url)?;
            verify_sha256(&bytes, &expected)
                .with_context(|| format!("{id}: integrity check"))?;
        }
        import_vsix(&bytes, vsx_root)
    }

    /// Only follow file URLs on the registry host itself.
    fn trusted(&self, url: &str) -> bool {
        url.starts_with(&format!("{}/", self.base))
    }
}

pub fn split_id(id: &str) -> Result<(&str, &str)> {
    let (ns, name) = id
        .split_once('.')
        .ok_or_else(|| anyhow!("expected <publisher>.<name>, got {id:?}"))?;
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            && !s.starts_with('.')
    };
    if !ok(ns) || !ok(name) {
        bail!("invalid extension id {id:?}");
    }
    Ok((ns, name))
}

pub fn parse_search(body: &str) -> Result<SearchResults> {
    let v: Value = serde_json::from_str(body).context("search response")?;
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        bail!("registry error: {e}");
    }
    Ok(SearchResults {
        total: v.get("totalSize").and_then(Value::as_u64).unwrap_or(0),
        offset: v.get("offset").and_then(Value::as_u64).unwrap_or(0),
        extensions: serde_json::from_value(
            v.get("extensions").cloned().unwrap_or(Value::Array(vec![])),
        )
        .context("search extensions")?,
    })
}

pub fn parse_detail(body: &str) -> Result<Extension> {
    let v: Value = serde_json::from_str(body).context("extension response")?;
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        bail!("registry error: {e}");
    }
    Ok(serde_json::from_value(v)?)
}

/// `expected` is the registry's `.sha256` file: a hex digest, optionally
/// followed by a file name.
pub fn verify_sha256(bytes: &[u8], expected: &str) -> Result<()> {
    let want = expected
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let got: String = Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if want.len() != 64 || want != got {
        bail!("sha256 mismatch (registry {want:?}, downloaded {got})");
    }
    Ok(())
}

/// Delete an installed extension. `id` is `publisher.name`; returns whether
/// anything was removed.
pub fn uninstall(id: &str, vsx_root: &Path) -> Result<bool> {
    split_id(id)?;
    let dir = vsx_root.join(id);
    if !dir.join("vsx.json").is_file() {
        return Ok(false);
    }
    fs::remove_dir_all(&dir).with_context(|| format!("remove {}", dir.display()))?;
    Ok(true)
}

/// Everything imported under `vsx_root`.
pub fn list_installed(vsx_root: &Path) -> Vec<Installed> {
    installed_dirs(vsx_root)
        .into_iter()
        .filter_map(|d: PathBuf| {
            let v: Value =
                serde_json::from_slice(&fs::read(d.join("vsx.json")).ok()?).ok()?;
            let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
            let n = |k: &str| v.get(k).and_then(Value::as_array).map_or(0, Vec::len);
            Some(Installed {
                id: s("id")?,
                version: s("version").unwrap_or_default(),
                display_name: s("display_name").unwrap_or_default(),
                license: s("license"),
                has_code: v
                    .get("has_code")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                color_themes: n("color_themes"),
                icon_themes: n("icon_themes"),
                grammars: n("grammars"),
                snippets: v.get("snippet_count").and_then(Value::as_u64).unwrap_or(0)
                    as usize,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search() {
        let r = parse_search(
            r#"{"offset":0,"totalSize":1,"extensions":[{"namespace":"a","name":"b","version":"1.0.0","displayName":"B","downloadCount":5,"files":{"download":"https://open-vsx.org/x.vsix"}}]}"#,
        )
        .unwrap();
        assert_eq!(r.total, 1);
        assert_eq!(r.extensions[0].id(), "a.b");
        assert_eq!(r.extensions[0].title(), "B");
        assert!(parse_search(r#"{"error":"nope"}"#).is_err());
    }

    #[test]
    fn id_validation() {
        assert!(split_id("dracula-theme.theme-dracula").is_ok());
        assert!(split_id("nodot").is_err());
        assert!(split_id("../x.y").is_err());
        assert!(split_id("a/b.c").is_err());
        assert!(split_id(".a.b").is_err());
    }

    #[test]
    fn sha() {
        let h = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verify_sha256(b"hello", &format!("{h}  f.vsix\n")).is_ok());
        assert!(verify_sha256(b"hellO", h).is_err());
    }

    #[test]
    fn uninstall_and_list() {
        let root =
            std::env::temp_dir().join(format!("vsx-test-{}", std::process::id()));
        let d = root.join("a.b");
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join("vsx.json"),
            r#"{"id":"a.b","version":"1","display_name":"B","color_themes":["x"],"has_code":true}"#,
        )
        .unwrap();
        let l = list_installed(&root);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].color_themes, 1);
        assert!(l[0].has_code);
        assert!(uninstall("a.b", &root).unwrap());
        assert!(!uninstall("a.b", &root).unwrap());
        assert!(uninstall("../a.b", &root).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
