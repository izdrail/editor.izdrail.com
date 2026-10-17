//! Hover a console command name - `Artisan::call('mail:send')`,
//! `$schedule->command('inspire')`, `$this->artisan('...')` - and see the
//! command's signature, description and source line, from the same static
//! scan as the "Go to Artisan Command" palette. Nothing is executed.

use std::path::{Path, PathBuf};

const MAX_LINE: usize = 4096;
const MAX_DEPTH: usize = 6;
const MAX_FILES: usize = 300;
const MAX_FILE: u64 = 256 * 1024;

const RECEIVERS: &[&str] = &[
    "Artisan::call(",
    "Artisan::callSilent(",
    "Artisan::queue(",
    "Artisan::command(",
    "Schedule::command(",
    "->command(",
    "$this->call(",
    "$this->artisan(",
];

/// First quoted string in `s` (single or double quotes).
fn quoted(s: &str) -> Option<&str> {
    let q = s.find(|c| c == '\'' || c == '"')?;
    let qc = s[q..].chars().next()?;
    let body = &s[q + 1..];
    let end = body.find(qc)?;
    Some(&body[..end])
}

/// The command name (first word of the quoted string) covering byte `col`,
/// gated on a call receiver.
fn name_at(line: &str, col: usize) -> Option<String> {
    if line.len() > MAX_LINE || !RECEIVERS.iter().any(|r| line.contains(r)) {
        return None;
    }
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
                let name = s.split_whitespace().next().unwrap_or("");
                let ok = !name.is_empty()
                    && name.len() <= 60
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '_'));
                return ok.then(|| name.to_string());
            }
        }
    }
    None
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_FILES {
        return;
    }
    if depth == 0
        && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink())
    {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.filter_map(Result::ok) {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() {
            walk(&p, depth + 1, out);
        } else if ft.is_file() && p.extension().is_some_and(|x| x == "php") {
            out.push(p);
        }
        if out.len() >= MAX_FILES {
            return;
        }
    }
}

/// `(signature-if-known, description, file, 0-based line)` for `name`.
fn find_command(root: &Path, name: &str) -> Option<(Option<String>, String, PathBuf, usize)> {
    // Closure commands in routes/console.php.
    let console = root.join("routes/console.php");
    if let Ok(md) = std::fs::metadata(&console) {
        if md.len() <= MAX_FILE {
            if let Ok(text) = std::fs::read_to_string(&console) {
                for cmd in crate::artisan_index::parse_closures(&text) {
                    if cmd.name == name {
                        return Some((None, cmd.description, console.clone(), cmd.line));
                    }
                }
            }
        }
    }
    // Class commands in app/Console/Commands.
    let mut files = Vec::new();
    walk(&root.join("app/Console/Commands"), 0, &mut files);
    for f in files {
        let Ok(md) = std::fs::symlink_metadata(&f) else { continue };
        if !md.is_file() || md.len() > MAX_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let Some(cmd) = crate::artisan_index::parse_class(&text) else { continue };
        if cmd.name != name {
            continue;
        }
        // Recover the full signature from its declaration line when the
        // `$signature` form declared it.
        let sig = text
            .lines()
            .nth(cmd.line)
            .filter(|l| l.contains("$signature"))
            .and_then(|l| quoted(&l[l.find("$signature")? + 10..]))
            .map(str::to_string);
        return Some((sig, cmd.description, f, cmd.line));
    }
    None
}

/// Markdown hover for the command name at byte `col` of `line`.
pub fn help(root: Option<&Path>, line: &str, col: usize) -> Option<String> {
    let root = root?;
    let name = name_at(line, col)?;
    let (sig, desc, file, idx) = find_command(root, &name)?;
    let rel = file.strip_prefix(root).unwrap_or(&file);
    let mut md = if desc.is_empty() {
        format!("**`{name}`**")
    } else {
        format!("**`{name}`** - {desc}")
    };
    if let Some(s) = sig.filter(|s| s != &name) {
        md.push_str(&format!("\n\n`{s}`"));
    }
    md.push_str(&format!("\n\n_{}:{}_", rel.display(), idx + 1));
    Some(md)
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
    fn commands_are_explained() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(
            r,
            "app/Console/Commands/SendMail.php",
            "<?php\nclass SendMail extends Command {\n    protected $signature = 'mail:send {user} {--force}';\n    protected $description = 'Send the mail';\n}\n",
        );
        put(
            r,
            "routes/console.php",
            "<?php\nuse Illuminate\\Support\\Facades\\Artisan;\nArtisan::command('inspire:me', function () {\n})->daily();\n",
        );
        let root = Some(r);

        let l = "Artisan::call('mail:send', ['user' => 1]);";
        let md = help(root, l, at(l, "mail:send")).unwrap();
        assert!(md.contains("**`mail:send`** - Send the mail"), "{md}");
        assert!(md.contains("`mail:send {user} {--force}`"), "{md}");
        assert!(md.contains("app/Console/Commands/SendMail.php:3"), "{md}");

        // Schedule form and closure commands in routes/console.php.
        let l = "$schedule->command('mail:send')->daily();";
        assert!(help(root, l, at(l, "mail:send")).is_some());
        let l = "Artisan::call('inspire:me');";
        let md = help(root, l, at(l, "inspire:me")).unwrap();
        assert!(md.contains("routes/console.php:3"), "{md}");

        // Unknown command, non-receiver, no root: nothing.
        let l = "Artisan::call('unknown:cmd');";
        assert_eq!(help(root, l, at(l, "unknown:cmd")), None);
        let l = "foo('mail:send')";
        assert_eq!(help(root, l, at(l, "mail:send")), None);
        assert_eq!(help(None, "Artisan::call('mail:send')", 16), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        let root = Some(d.path());
        for l in ["Artisan::call('", "'", "call('é')", "call('日本')", "->command("] {
            for c in 0..=l.len() + 2 {
                let _ = help(root, l, c);
            }
        }
    }
}
