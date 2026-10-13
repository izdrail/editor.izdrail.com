use std::io::{Cursor, Read};

use anyhow::{Context, Result, bail};
use zip::ZipArchive;

use crate::manifest::Manifest;

pub const MAX_VSIX_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_ENTRY_BYTES: u64 = 32 * 1024 * 1024;

/// A `.vsix` package opened for reading. Nothing in it is ever executed.
pub struct Vsix {
    archive: ZipArchive<Cursor<Vec<u8>>>,
    pub manifest: Manifest,
}

/// Normalise a path written inside an extension (relative to the extension
/// root). Returns `None` when it escapes the root.
pub fn normalize_rel(rel: &str) -> Option<String> {
    let rel = rel.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for part in rel.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// Resolve `rel` against the directory containing `base` (both root-relative).
pub fn resolve_rel(base_file: &str, rel: &str) -> Option<String> {
    let dir = base_file.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    normalize_rel(&format!("{dir}/{rel}"))
}

impl Vsix {
    pub fn open(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() > MAX_VSIX_BYTES {
            bail!("vsix is larger than {} MiB", MAX_VSIX_BYTES >> 20);
        }
        let mut archive =
            ZipArchive::new(Cursor::new(bytes)).context("not a zip archive")?;
        if archive.len() > 50_000 {
            bail!("vsix has too many entries");
        }
        let read = |archive: &mut ZipArchive<Cursor<Vec<u8>>>, name: &str| {
            let mut f = archive.by_name(name).ok()?;
            if f.size() > MAX_ENTRY_BYTES {
                return None;
            }
            let mut s = String::new();
            f.read_to_string(&mut s).ok()?;
            Some(s)
        };
        let pkg = read(&mut archive, "extension/package.json")
            .context("extension/package.json missing")?;
        let nls = read(&mut archive, "extension/package.nls.json");
        let manifest = Manifest::parse(&pkg, nls.as_deref())?;
        Ok(Self { archive, manifest })
    }

    /// Read a file by its path relative to the extension root.
    pub fn read(&mut self, rel: &str) -> Result<Vec<u8>> {
        let rel = normalize_rel(rel)
            .with_context(|| format!("path escapes extension root: {rel}"))?;
        let name = format!("extension/{rel}");
        let idx = match self.archive.index_for_name(&name) {
            Some(i) => i,
            None => (0..self.archive.len())
                .find(|i| {
                    self.archive
                        .name_for_index(*i)
                        .map(|n| n.eq_ignore_ascii_case(&name))
                        .unwrap_or(false)
                })
                .with_context(|| format!("{name} not in vsix"))?,
        };
        let mut file = self.archive.by_index(idx)?;
        if file.size() > MAX_ENTRY_BYTES {
            bail!("{name} is too large");
        }
        let mut buf = Vec::with_capacity(file.size() as usize);
        file.by_ref().take(MAX_ENTRY_BYTES).read_to_end(&mut buf)?;
        Ok(buf)
    }

    pub fn read_text(&mut self, rel: &str) -> Result<String> {
        Ok(String::from_utf8_lossy(&self.read(rel)?).into_owned())
    }

    pub fn read_json(&mut self, rel: &str) -> Result<serde_json::Value> {
        crate::jsonc::parse(&self.read_text(rel)?)
    }

    pub fn license_text(&mut self) -> Option<String> {
        for n in ["LICENSE", "LICENSE.txt", "LICENSE.md", "LICENSE-vscode.txt"] {
            if let Ok(t) = self.read_text(n) {
                return Some(t);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_paths() {
        assert_eq!(normalize_rel("./a/b.json").as_deref(), Some("a/b.json"));
        assert_eq!(normalize_rel("a/../b").as_deref(), Some("b"));
        assert_eq!(normalize_rel("../x"), None);
        assert_eq!(normalize_rel("a/../../x"), None);
        assert_eq!(
            resolve_rel("themes/a.json", "../icons/x.svg").as_deref(),
            Some("icons/x.svg")
        );
    }
}
