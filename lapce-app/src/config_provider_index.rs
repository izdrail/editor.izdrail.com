//! Index for the "Go to Config / Service Provider" palette.
//!
//! - `config/**.php`: one `[Config] name` entry per file (`config/app.php` ->
//!   `app`, `config/services/x.php` -> `services.x`), the name you pass to
//!   `config('...')`.
//! - `app/Providers/**.php`: `[Provider] Class`, plus its `register` and
//!   `boot` methods when present.
//!
//! Static text scan, nothing is executed. Bounded: depth 4 for config and 6
//! for providers, 1000 files in total, 128 KB read per provider, 3000
//! entries, symlinks skipped.
use std::{
    io::Read,
    path::{Path, PathBuf},
};

use crate::route_index::RouteTarget;

const MAX_FILES: usize = 1000;
const MAX_ENTRIES: usize = 3000;
const HEAD_BYTES: u64 = 128 * 1024;

fn walk(dir: &Path, depth: usize, max_depth: usize, out: &mut Vec<PathBuf>) {
    if depth > max_depth || out.len() >= MAX_FILES {
        return;
    }
    if depth == 0 && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink()) {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if out.len() >= MAX_FILES {
            return;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() {
            walk(&p, depth + 1, max_depth, out);
        } else if ft.is_file() && p.extension().is_some_and(|x| x == "php") {
            out.push(p);
        }
    }
}

/// `config/services/x.php` relative to `config/` -> `services.x`.
pub fn config_name(rel: &Path) -> Option<String> {
    let parts: Vec<&str> = rel.components().filter_map(|c| c.as_os_str().to_str()).collect();
    let (last, dirs) = parts.split_last()?;
    let stem = last.strip_suffix(".php")?;
    if stem.is_empty() {
        return None;
    }
    let mut name: Vec<&str> = dirs.to_vec();
    name.push(stem);
    Some(name.join("."))
}

/// `(class, class line, register line, boot line)` of a provider file.
pub fn parse_provider(text: &str) -> Option<(String, usize, Option<usize>, Option<usize>)> {
    let mut class: Option<(String, usize)> = None;
    let (mut register, mut boot) = (None, None);
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with("//") || line.starts_with('*') || line.starts_with('#') || line.starts_with("/*") {
            continue;
        }
        if class.is_none() {
            let mut w = line.split_whitespace();
            let mut first = w.next();
            while matches!(first, Some("final" | "abstract" | "readonly")) {
                first = w.next();
            }
            if first == Some("class") {
                let name: String = w.next()?.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                if name.is_empty() {
                    return None;
                }
                class = Some((name, i));
            }
            continue;
        }
        if let Some(p) = line.find("function ") {
            let name = line[p + 9..].trim_start();
            for (n, slot) in [("register", &mut register), ("boot", &mut boot)] {
                if slot.is_none() && name.strip_prefix(n).is_some_and(|r| r.trim_start().starts_with('(')) {
                    *slot = Some(i);
                }
            }
        }
    }
    class.map(|(c, l)| (c, l, register, boot))
}

fn head(p: &Path) -> Option<String> {
    let mut buf = Vec::new();
    std::fs::File::open(p).ok()?.take(HEAD_BYTES).read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

pub fn discover(root: &Path) -> Vec<RouteTarget> {
    let mut out = Vec::new();
    let shown = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string();

    let cfg_dir = root.join("config");
    let mut files = Vec::new();
    walk(&cfg_dir, 0, 4, &mut files);
    for f in files {
        let Some(name) = f.strip_prefix(&cfg_dir).ok().and_then(config_name) else { continue };
        out.push(RouteTarget { label: format!("[Config] {name}  {}", shown(&f)), path: f, line: 0 });
    }

    let mut files = Vec::new();
    walk(&root.join("app/Providers"), 0, 6, &mut files);
    for f in files {
        if out.len() >= MAX_ENTRIES {
            break;
        }
        let Some((class, line, register, boot)) = head(&f).and_then(|t| parse_provider(&t)) else { continue };
        out.push(RouteTarget { label: format!("[Provider] {class}  {}", shown(&f)), path: f.clone(), line });
        for (m, l) in [("register", register), ("boot", boot)] {
            if let Some(l) = l {
                out.push(RouteTarget { label: format!("[Provider] {class}::{m}()"), path: f.clone(), line: l });
            }
        }
    }
    out.truncate(MAX_ENTRIES);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn config_names() {
        assert_eq!(config_name(Path::new("app.php")).as_deref(), Some("app"));
        assert_eq!(config_name(Path::new("services/x.php")).as_deref(), Some("services.x"));
        assert_eq!(config_name(Path::new(".php")), None);
        assert_eq!(config_name(Path::new("a.txt")), None);
        assert_eq!(config_name(Path::new("")), None);
    }

    #[test]
    fn provider_parse() {
        let t = "<?php\nnamespace App\\Providers;\n\nclass AppServiceProvider extends ServiceProvider\n{\n    public function register(): void\n    {\n    }\n\n    public function boot(): void {}\n    private function bootstrap() {}\n}\n";
        assert_eq!(parse_provider(t), Some(("AppServiceProvider".into(), 3, Some(5), Some(9))));
        assert_eq!(parse_provider("<?php\n// class X\nfinal class P {}"), Some(("P".into(), 2, None, None)));
        assert_eq!(parse_provider("<?php\n$x = 1;"), None);
        assert_eq!(parse_provider("class "), None);
        assert_eq!(parse_provider("class é"), None);
    }

    #[test]
    fn discovers_configs_and_providers() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "config/app.php", "<?php return [];");
        put(r, "config/services/mail.php", "<?php return [];");
        put(r, "config/notes.txt", "x");
        put(r, "app/Providers/AppServiceProvider.php", "class AppServiceProvider {\n function boot() {}\n}");
        put(r, "app/Providers/Empty.php", "<?php\n");
        let got = discover(r);
        let labels: Vec<_> = got.iter().map(|t| t.label.split("  ").next().unwrap().to_string()).collect();
        assert_eq!(labels, ["[Config] app", "[Config] services.mail", "[Provider] AppServiceProvider", "[Provider] AppServiceProvider::boot()"]);
        assert_eq!(got[3].line, 1);
        assert!(discover(&r.join("none")).is_empty());
    }

    #[test]
    fn bounds_and_symlinks() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        for i in 0..1200 {
            put(r, &format!("config/c{i}.php"), "<?php");
        }
        assert!(discover(r).len() <= MAX_FILES);
        #[cfg(unix)]
        {
            let d2 = tempfile::tempdir().unwrap();
            put(&d2.path().join("real"), "a.php", "<?php");
            std::os::unix::fs::symlink(d2.path().join("real"), d2.path().join("config")).unwrap();
            assert!(discover(d2.path()).is_empty());
        }
    }

    #[test]
    fn random_text_never_panics() {
        let pieces = ["class ", "final ", "function ", "register", "boot", "(", ")", "é", "日", "\n", "//", " ", "{", "x"];
        let mut s = 13u64;
        for _ in 0..500 {
            let mut t = String::new();
            for _ in 0..60 {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                t.push_str(pieces[(s >> 33) as usize % pieces.len()]);
            }
            let _ = parse_provider(&t);
        }
    }
}
