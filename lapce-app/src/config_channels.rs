//! Go to the config entry behind a named Laravel driver: `Storage::disk('s3')`,
//! `Log::channel('slack')`, `Cache::store('redis')`, `Mail::mailer('ses')`,
//! `Auth::guard('api')`, `Auth::shouldUse('web')`, `DB::connection('mysql')`,
//! `Queue::connection('sqs')`, `Redis::connection('cache')`, `->disk('s3')`.
//! Each opens the `'s3' => [...]` line in the matching `config/*.php` file
//! (`filesystems`, `logging`, `cache`, `mail`, `auth`, `database`, `queue`).
//!
//! Textual only; nothing is executed. The first `'name' =>` line wins, so a
//! name reused inside one config file may open an earlier entry. Bounded: 1 MB
//! per file. Config published under a different path is not found.
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 1_000_000;

/// `(method, receiver filter, config file)`; an empty filter accepts any receiver.
const CALLS: &[(&str, &[&str], &str)] = &[
    ("disk", &[], "filesystems"),
    ("channel", &[], "logging"),
    ("store", &[], "cache"),
    ("mailer", &[], "mail"),
    ("guard", &[], "auth"),
    ("shouldUse", &[], "auth"),
    ("connection", &["DB"], "database"),
    ("connection", &["Redis"], "database"),
    ("connection", &["Queue"], "queue"),
];

fn target_at(line: &str, col: usize) -> Option<(&str, &'static str)> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let i = line[..col].rfind(['\'', '"'])?;
    let q = line[i..].chars().next()?;
    let end = col + line[col..].find(q)?;
    let name = &line[i + 1..end];
    if name.is_empty()
        || name.len() > 60
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return None;
    }
    let before = line[..i].trim_end().strip_suffix('(')?.trim_end();
    CALLS.iter().find_map(|(m, recv, file)| {
        let rest = before.strip_suffix(m)?;
        let rest = rest.strip_suffix("::").or_else(|| rest.strip_suffix("->"))?;
        let word_start = rest
            .char_indices()
            .rev()
            .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_' || *c == '\\'))
            .map_or(0, |(p, c)| p + c.len_utf8());
        let receiver = rest[word_start..].rsplit('\\').next().unwrap_or("");
        (recv.is_empty() || recv.contains(&receiver)).then_some((name, *file))
    })
}

/// Config file and 0-based line of the entry for the name under `col`.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let (name, file) = target_at(line, col)?;
    let p = root.join("config").join(format!("{file}.php"));
    let meta = std::fs::metadata(&p).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(&p).ok()?;
    let i = text.lines().position(|l| {
        ['\'', '"'].iter().any(|q| {
            let key = format!("{q}{name}{q}");
            l.match_indices(&key).any(|(i, _)| l[i + key.len()..].trim_start().starts_with("=>"))
        })
    })?;
    Some((p, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, text: &str) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    #[test]
    fn named_drivers_open_their_config_entry() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let fs = put(r, "config/filesystems.php", "<?php\nreturn [\n    'disks' => [\n        's3' => [\n        ],\n    ],\n];");
        let db = put(r, "config/database.php", "<?php\nreturn [\n    'connections' => [\n        'mysql' => [],\n    ],\n    'redis' => [\n        'cache' => [],\n    ],\n];");
        let q = put(r, "config/queue.php", "<?php\nreturn [\n    'connections' => [\n        'sqs' => [],\n    ],\n];");
        let go = |l: &str, n: &str| resolve(r, l, at(l, n));
        let l = "Storage::disk('s3')->put($a, $b);";
        assert_eq!(go(l, "s3"), Some((fs.clone(), 3)));
        let l = "$x = \\Illuminate\\Support\\Facades\\Storage::disk(\"s3\");";
        assert_eq!(go(l, "s3"), Some((fs, 3)));
        let l = "DB::connection('mysql')->table('a');";
        assert_eq!(go(l, "mysql"), Some((db.clone(), 3)));
        let l = "Redis::connection('cache')->get('k');";
        assert_eq!(go(l, "cache"), Some((db, 6)));
        let l = "Queue::connection('sqs')->size();";
        assert_eq!(go(l, "sqs"), Some((q, 3)));
    }

    #[test]
    fn unrelated_strings_are_ignored() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "config/filesystems.php", "<?php\n's3' => [],\n");
        put(r, "config/queue.php", "<?php\n'sqs' => [],\n");
        for (l, n) in [
            ("foo('s3')", "s3"),
            ("Storage::disk('local')", "local"),
            ("Foo::connection('sqs')", "sqs"),
            ("Storage::disk($d, 's3')", "s3"),
            ("mydisk('s3')", "s3"),
        ] {
            assert_eq!(resolve(r, l, at(l, n)), None, "{l}");
        }
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        for l in ["'", "disk('", "disk('é", "", "::disk('日本')", "->->('a')"] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), l, c);
            }
        }
    }
}
