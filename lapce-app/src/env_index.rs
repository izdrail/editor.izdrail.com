//! Index for the "Go to Env / Config Key" palette: every variable defined in
//! `.env`, `.env.local`, `.env.example`, `.env.dist` (names only, values are
//! never shown) and every Laravel `config/*.php` key up to two levels deep
//! (`app.name`, `database.connections`).
//!
//! Static text scan, nothing is executed. Bounded: 1 MB per file, 40 config
//! files, 5000 entries, symlinks skipped.
use std::path::{Path, PathBuf};

use crate::route_index::RouteTarget;

const MAX_FILE: u64 = 1 << 20;
const MAX_CONFIG_FILES: usize = 40;
const MAX_ENTRIES: usize = 5000;
const ENV_FILES: &[&str] = &[".env", ".env.local", ".env.example", ".env.dist"];

fn read(path: &Path) -> Option<String> {
    let md = std::fs::symlink_metadata(path).ok()?;
    if !md.is_file() || md.len() > MAX_FILE {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// `(KEY, 0-based line)` for each `KEY=...` line.
pub fn parse_env(text: &str) -> Vec<(String, usize)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let l = l.trim_start();
        let l = l.strip_prefix("export ").unwrap_or(l).trim_start();
        if l.starts_with('#') {
            continue;
        }
        if let Some((k, _)) = l.split_once('=') {
            let k = k.trim();
            if !k.is_empty()
                && k.len() <= 100
                && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                out.push((k.to_string(), i));
            }
        }
    }
    out
}

/// `(dotted.key, 0-based line)` for string keys at depth 1 and 2 of the array
/// returned by a Laravel config file. Brackets inside strings and comments are
/// ignored.
pub fn parse_config(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut in_ret = false;
    let mut path: Vec<String> = Vec::new();
    let mut block_comment = false;
    for (i, line) in text.lines().enumerate() {
        let b = line.as_bytes();
        let mut j = 0;
        let mut last_str: Option<String> = None;
        while j < b.len() {
            if block_comment {
                if line[j..].starts_with("*/") {
                    block_comment = false;
                    j += 2;
                } else {
                    j += 1;
                }
                continue;
            }
            match b[j] {
                b'/' if line[j..].starts_with("//") => break,
                b'#' => break,
                b'/' if line[j..].starts_with("/*") => {
                    block_comment = true;
                    j += 2;
                }
                q @ (b'\'' | b'"') => {
                    let mut k = j + 1;
                    let mut s = Vec::new();
                    while k < b.len() && b[k] != q {
                        if b[k] == b'\\' && k + 1 < b.len() {
                            k += 1;
                        }
                        s.push(b[k]);
                        k += 1;
                    }
                    last_str = Some(String::from_utf8_lossy(&s).into_owned());
                    j = k + 1;
                    // `'key' =>` at depth 1 or 2 inside the returned array.
                    if in_ret && (depth == 1 || depth == 2) && line[j.min(line.len())..].trim_start().starts_with("=>") {
                        let key = last_str.clone().unwrap_or_default();
                        if !key.is_empty() && key.len() <= 100 {
                            path.truncate(depth - 1);
                            path.push(key);
                            out.push((path.join("."), i));
                        }
                    }
                }
                b'[' => {
                    if !in_ret && line[..j].contains("return") {
                        in_ret = true;
                    }
                    depth += 1;
                    j += 1;
                }
                b']' => {
                    depth = depth.saturating_sub(1);
                    if in_ret && depth == 0 {
                        return out;
                    }
                    j += 1;
                }
                _ => j += 1,
            }
        }
    }
    out
}

pub fn discover(root: &Path) -> Vec<RouteTarget> {
    let mut out: Vec<RouteTarget> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for name in ENV_FILES {
        let path = root.join(name);
        let Some(text) = read(&path) else { continue };
        for (key, line) in parse_env(&text) {
            if seen.insert(key.clone()) {
                out.push(RouteTarget { label: format!("env  {key}  ({name})"), path: path.clone(), line });
            }
        }
    }
    let dir = root.join("config");
    if std::fs::symlink_metadata(&dir).map_or(false, |m| m.is_dir()) {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "php"))
            .collect();
        files.sort();
        files.truncate(MAX_CONFIG_FILES);
        for f in files {
            let Some(text) = read(&f) else { continue };
            let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            for (key, line) in parse_config(&text) {
                out.push(RouteTarget {
                    label: format!("config  {stem}.{key}  (config/{stem}.php)"),
                    path: f.clone(),
                    line,
                });
            }
        }
    }
    out.truncate(MAX_ENTRIES);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_lines() {
        let t = "# c\nAPP_NAME=Laravel\n export DB_HOST = x\nbad key=1\n\nAPP_KEY=\n=x\n";
        assert_eq!(
            parse_env(t),
            vec![("APP_NAME".into(), 1), ("DB_HOST".into(), 2), ("APP_KEY".into(), 5)]
        );
        assert_eq!(parse_env("\u{feff}A=1"), vec![("A".into(), 0)]);
    }

    #[test]
    fn config_keys_two_levels() {
        let t = "<?php\n\nreturn [\n    // 'ignored' => 1,\n    'name' => env('APP_NAME', 'Laravel'),\n    'providers' => [\n        App\\P::class,\n        'x' => ']',\n    ],\n    /* 'nope' => 1 */\n    'connections' => [\n        'mysql' => [\n            'host' => 'h',\n        ],\n    ],\n];\n";
        let k: Vec<_> = parse_config(t).into_iter().collect();
        assert_eq!(
            k,
            vec![
                ("name".into(), 4),
                ("providers".into(), 5),
                ("providers.x".into(), 7),
                ("connections".into(), 10),
                ("connections.mysql".into(), 11),
            ]
        );
        assert!(parse_config("").is_empty());
        let _ = parse_config("return [ 'a' => [");
        let _ = parse_config("'é' => \"é\\");
    }

    #[test]
    fn discover_end_to_end_hides_values() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::write(r.join(".env"), "APP_NAME=secret-value\nDB=1\n").unwrap();
        std::fs::write(r.join(".env.example"), "APP_NAME=\nEXTRA=1\n").unwrap();
        std::fs::create_dir(r.join("config")).unwrap();
        std::fs::write(r.join("config/app.php"), "<?php\nreturn [\n 'name' => 'x',\n];\n").unwrap();
        let got = discover(r);
        let labels: Vec<_> = got.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "env  APP_NAME  (.env)",
                "env  DB  (.env)",
                "env  EXTRA  (.env.example)",
                "config  app.name  (config/app.php)"
            ]
        );
        assert!(!labels.iter().any(|l| l.contains("secret-value")));
        assert!(discover(&r.join("missing")).is_empty());
    }
}
