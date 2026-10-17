//! Hover a named Laravel driver: `Storage::disk('s3')`, `Log::channel(...)`,
//! `Cache::store(...)`, `Mail::mailer(...)`, `Auth::guard(...)`,
//! `DB::connection(...)` and friends show the entry's driver (or mail
//! transport) from `config/*.php`, with the file and line. Reuses
//! `config_channels` for resolution, so callers and limits match the go-to.
//! Static text scan; nothing is executed.

use std::path::Path;

const MAX_LINE: usize = 4096;
const MAX_FILE: u64 = 1 << 20;
/// Lines scanned past the entry for the driver key.
const SCAN: usize = 12;

/// The quoted string content covering byte `col`.
fn name_at(line: &str, col: usize) -> Option<String> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    for q in ['\'', '"'] {
        for (i, _) in line.match_indices(q) {
            let start = i + 1;
            let Some(len) = line[start..].find(q) else { break };
            let end = start + len;
            if col >= start && col <= end {
                let s = &line[start..end];
                return (s.len() <= 60).then(|| s.to_string());
            }
        }
    }
    None
}

fn label(stem: &str) -> &str {
    match stem {
        "filesystems" => "filesystem disk",
        "logging" => "log channel",
        "cache" => "cache store",
        "mail" => "mailer",
        "auth" => "auth guard",
        "database" => "database connection",
        "queue" => "queue connection",
        _ => "config entry",
    }
}

/// `'key' => 'value'` (or `=> Something::class`) on a line.
fn entry_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let i = line.find(&format!("'{key}'"))?;
    let mut rest = line[i..].split_once("=>")?.1.trim();
    while let Some(r) = rest.strip_suffix([',', ']', '}']) {
        rest = r.trim_end();
    }
    Some(rest.trim_matches(['\'', '"']))
}

/// Markdown hover for the driver name at byte `col` of `line`.
pub fn help(root: Option<&Path>, line: &str, col: usize) -> Option<String> {
    let root = root?;
    if line.len() > MAX_LINE {
        return None;
    }
    let name = name_at(line, col)?;
    let (path, idx) = crate::config_channels::resolve(root, line, col)?;
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() > MAX_FILE {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    // The entry may be one line or the head of a nested array; scan a small
    // window for the driver (mail entries use `transport` instead).
    let driver = lines
        .iter()
        .skip(idx)
        .take(SCAN)
        .find_map(|&l| entry_value(l, "driver").or_else(|| entry_value(l, "transport")));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let rel = path.strip_prefix(root).unwrap_or(&path);
    let what = match driver {
        Some(d) => format!("{}, driver `{d}`", label(stem)),
        None => label(stem).to_string(),
    };
    Some(format!(
        "**`{name}`** - {what}\n\n_{}:{}\n\nFirst matching entry wins._",
        rel.display(),
        idx + 1,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
    }

    fn at(line: &str, needle: &str) -> usize {
        line.rfind(needle).unwrap() + 1
    }

    #[test]
    fn drivers_are_explained() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "config/filesystems.php", "<?php\nreturn [\n    'disks' => [\n        's3' => [\n            'driver' => 's3',\n            'root' => env('AWS_ROOT'),\n        ],\n    ],\n];\n");
        put(r, "config/mail.php", "<?php\nreturn [\n    'mailers' => [\n        'ses' => ['transport' => 'ses'],\n    ],\n];\n");
        let root = Some(r);

        let l = "Storage::disk('s3')->put($a, $b);";
        let md = help(root, l, at(l, "s3")).unwrap();
        assert!(md.contains("**`s3`** - filesystem disk, driver `s3`"), "{md}");
        assert!(md.contains("config/filesystems.php:4"), "{md}");

        // Single-line entry, and mail uses `transport`.
        let l = "Mail::mailer('ses')->send($m);";
        let md = help(root, l, at(l, "ses")).unwrap();
        assert!(md.contains("mailer, driver `ses`"), "{md}");

        // Unknown name and non-driver calls: nothing.
        let l = "Storage::disk('local')->put($a, $b);";
        assert_eq!(help(root, l, at(l, "local")), None);
        let l = "foo('s3')";
        assert_eq!(help(root, l, at(l, "s3")), None);
        assert_eq!(help(None, "Storage::disk('s3')", 18), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        let root = Some(d.path());
        for l in ["Storage::disk('", "'", "\"", "disk('é')", "disk('日本')", "()"] {
            for c in 0..=l.len() + 2 {
                let _ = help(root, l, c);
            }
        }
    }
}
