//! Hover a named Laravel driver: `Storage::disk('s3')`, `Log::channel(...)`,
//! `Cache::store(...)`, `Mail::mailer(...)`, `Auth::guard(...)`,
//! `DB::connection(...)` and friends show the entry's driver (or mail
//! transport) from `config/*.php`, with the file and line. Reuses
//! `config_channels` for resolution, so callers and limits match the go-to.
//! Static text scan; nothing is executed.

use std::path::Path;

const MAX_LINE: usize = 4096;
const MAX_FILE: u64 = 1 << 20;
/// The quoted string content covering byte `col`.
fn name_at(line: &str, col: usize) -> Option<String> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    for q in ['\'', '"'] {
        let quotes: Vec<usize> = line.match_indices(q).map(|(i, _)| i).collect();
        for pair in quotes.chunks_exact(2) {
            let start = pair[0] + 1;
            let end = pair[1];
            if col + 1 >= start && col <= end {
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
    let driver = lapce_index::symbols::php_array_property(&text, idx, &name, &["driver", "transport"])
        .ok().flatten();
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let rel = path.strip_prefix(root).unwrap_or(&path);
    let what = match driver {
        Some(d) => format!("{}, driver `{}`", label(stem), d.trim_matches(['\'', '"'])),
        None => label(stem).to_string(),
    };
    Some(format!(
        "**`{name}`** - {what}\n\n_{}:{}_\n\n_First matching entry wins._",
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
    fn quotes_count_as_inside() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "config/filesystems.php", "<?php
return [
    'disks' => [
        's3' => ['driver' => 's3'],
    ],
];
");
        let root = Some(r);
        let l = "Storage::disk('s3')->put(, );";
        let open = l.find("'s3'").unwrap();
        let expect = help(root, l, open + 2);
        assert_eq!(help(root, l, open), expect, "opening quote");
        assert_eq!(help(root, l, open + 3), expect, "closing quote");
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
        assert!(md.contains("_config/filesystems.php:4_\n\n_First matching entry wins._"), "{md}");

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
    fn driver_belongs_to_the_selected_config_array() {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "config/filesystems.php", r#"<?php
return ['disks' => [
    'custom' => [
        'root' => '/tmp/custom',
    ],
    'local' => ['driver' => 'local', 'root' => '/tmp'],
    'remote' => ["driver" => "s3", "bucket" => "assets"],
]];
"#);
        let line = "Storage::disk('custom')";
        let md = help(Some(d.path()), line, at(line, "custom")).unwrap();
        assert!(!md.contains("driver `"), "{md}");
        for (name, driver) in [("local", "local"), ("remote", "s3")] {
            let line = format!("Storage::disk('{name}')");
            let md = help(Some(d.path()), &line, at(&line, name)).unwrap();
            assert!(md.contains(&format!("driver `{driver}`")), "{md}");
            assert!(!md.contains("root") && !md.contains("bucket"), "{md}");
        }
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
