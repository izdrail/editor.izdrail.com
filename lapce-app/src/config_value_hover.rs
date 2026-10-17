//! Hover the value behind a Laravel config or translation key:
//! `config('mail.from.address')` shows the `'address' =>` value from
//! `config/mail.php`, `__('messages.welcome')` / `trans(...)` / `@lang(...)`
//! shows the translation line for the first locale that has it. Uses the
//! same callers, resolution and limits as `laravel_keys`. Static text scan;
//! nothing is executed.

use std::path::Path;

const MAX_LINE: usize = 4096;
const MAX_FILE: u64 = 1 << 20;
const MAX_VALUE: usize = 140;

/// The quoted string content covering byte `col`, if it is a key-like token.
fn key_at(line: &str, col: usize) -> Option<String> {
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
                return (s.len() <= 200).then(|| s.to_string());
            }
        }
    }
    None
}

/// Markdown hover for the config/translation key at byte `col` of `line`.
pub fn help(root: Option<&Path>, line: &str, col: usize) -> Option<String> {
    let root = root?;
    if line.len() > MAX_LINE {
        return None;
    }
    let key = key_at(line, col)?;
    let (path, idx) = crate::laravel_keys::resolve(root, line, col)?;
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() > MAX_FILE {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let found = text.lines().nth(idx)?.trim();
    // The found line must be the deepest requested key, not a shallower
    // prefix (an unknown leaf resolves to its parent line).
    if let Some(leaf) = key.rsplit('.').next() {
        let quoted = [format!("'{leaf}'"), format!("\"{leaf}\"")];
        if !quoted.iter().any(|q| found.contains(q.as_str())) {
            return None;
        }
    }
    let json = path.extension().is_some_and(|e| e == "json");
    let raw = if json {
        found.split_once(':')?.1
    } else {
        found.split_once("=>")?.1
    };
    let value = raw.trim().strip_suffix(',').unwrap_or(raw.trim()).trim();
    if value.is_empty() {
        return None;
    }
    let value = if value == "[" {
        "[ ... ] (nested array)".to_string()
    } else {
        value.chars().take(MAX_VALUE).collect()
    };
    let rel = path.strip_prefix(root).unwrap_or(&path);
    Some(format!(
        "**`{key}`** = `{value}`\n\n_{}:{}\n\nFirst matching locale / first entry wins._",
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
        line.find(needle).unwrap() + 1
    }

    #[test]
    fn config_and_lang_values() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "config/mail.php", "<?php\nreturn [\n  'default' => 'smtp',\n  'from' => [\n    'address' => 'a@x.test',\n    'name' => env('MAIL_FROM_NAME', 'Example'),\n  ],\n];\n");
        put(r, "lang/en/messages.php", "<?php\nreturn [\n  'welcome' => 'Hi there',\n];\n");
        put(r, "lang/en.json", "{\n  \"Welcome back\": \"Hi\"\n}\n");
        let root = Some(r);

        let l = "$m = config('mail.from.address');";
        let md = help(root, l, at(l, "mail")).unwrap();
        assert!(md.contains("**`mail.from.address`** = `'a@x.test'`"), "{md}");
        assert!(md.contains("config/mail.php:5"), "{md}");

        let l = "$m = config('mail.from.name');";
        assert!(help(root, l, at(l, "from")).unwrap().contains("env('MAIL_FROM_NAME', 'Example')"));

        // Nested array: pointed to, not dumped.
        let l = "$m = config('mail.from');";
        assert!(help(root, l, at(l, "mail")).unwrap().contains("nested array"));

        // Unknown leaf: no hover rather than the parent's value.
        let l = "$m = config('mail.from.nope');";
        assert_eq!(help(root, l, at(l, "mail")), None);
        // Whole file key: nothing to show.
        let l = "$m = config('mail');";
        assert_eq!(help(root, l, at(l, "mail")), None);

        let l = "{{ __('messages.welcome') }}";
        assert!(help(root, l, at(l, "messages")).unwrap().contains("'Hi there'"));
        let l = "<p>{{ __('Welcome back') }}</p>";
        assert!(help(root, l, at(l, "Welcome")).unwrap().contains("= `\"Hi\"`"));

        // No call, no workspace: nothing.
        let l = "$m = thing('mail.from.address');";
        assert_eq!(help(root, l, at(l, "mail")), None);
        let l = "$m = config('mail.from.address');";
        assert_eq!(help(None, l, at(l, "mail")), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        let root = Some(d.path());
        for l in ["'", "\"", "config('", "config('é.x')", "config('日本')", "config('')", "__('a.b')"] {
            for c in 0..=l.len() + 2 {
                let _ = help(root, l, c);
            }
        }
        let long = format!("config('{}')", "a.".repeat(3000));
        for c in 0..=long.len() {
            let _ = help(root, &long, c);
        }
    }
}
