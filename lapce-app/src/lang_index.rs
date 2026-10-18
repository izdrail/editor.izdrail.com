//! Index for the "Go to Translation" palette: every translation key defined
//! under `resources/lang` - PHP group files (`resources/lang/en/auth.php`,
//! nested arrays flattened with dot notation: `auth.throttle.seconds`) and
//! JSON files (`resources/lang/en.json`, flat keys). String escapes in keys
//! are decoded (PHP string rules for .php, JSON rules for .json), so the
//! listed key matches what __() and trans() expect.
//!
//! Static text scan, nothing is executed. Bounded: 1 MB per file, 20 locale
//! directories, 100 PHP files, 5000 entries, symlinks skipped. Only the
//! modern `[...]` array syntax is parsed (the Laravel 9+ skeleton style);
//! `array(...)` group files are skipped. A key defined in several locales is
//! listed once, for the alphabetically first locale.
use std::path::{Path, PathBuf};

use crate::route_index::RouteTarget;

const MAX_FILE: u64 = 1 << 20;
const MAX_LOCALES: usize = 20;
const MAX_FILES: usize = 100;
const MAX_ENTRIES: usize = 5000;
/// Deepest PHP array nesting flattened into the dotted key.
const MAX_DEPTH: usize = 4;

fn read(path: &Path) -> Option<String> {
    let md = std::fs::symlink_metadata(path).ok()?;
    if !md.is_file() || md.len() > MAX_FILE {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Decode a JSON string body (the bytes between the quotes): \" \\ \/ \n
/// \r \t \b \f and \uXXXX with surrogate pairs. The decoded key is what
/// Laravel's json_decode produces and what __() expects. An unknown escape
/// keeps its backslash; a truncated \u keeps its bytes; a broken surrogate
/// pair becomes U+FFFD.
fn json_unescape(raw: &str) -> String {
    if !raw.contains('\\') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('/') => out.push('/'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('b') => out.push('\u{0008}'),
            Some('f') => out.push('\u{000C}'),
            Some('u') => {
                let hi = take_hex4(&mut chars);
                match hi {
                    Some(h) if (0xD800..0xDC00).contains(&h) => {
                        // High surrogate: pair with the low half.
                        let bs = chars.next();
                        let u = chars.next();
                        let lo = if bs == Some('\\') && u == Some('u') {
                            take_hex4(&mut chars)
                        } else {
                            None
                        };
                        match lo {
                            Some(l) if (0xDC00..0xE000).contains(&l) => {
                                let c = 0x10000 + ((h - 0xD800) << 10) + (l - 0xDC00);
                                out.push(char::from_u32(c).unwrap_or('\u{FFFD}'));
                            }
                            _ => out.push('\u{FFFD}'),
                        }
                    }
                    Some(h) => {
                        out.push(char::from_u32(h).unwrap_or('\u{FFFD}'));
                    }
                    None => out.push_str("\\u"),
                }
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn take_hex4(chars: &mut std::str::Chars) -> Option<u32> {
    let mut v = 0u32;
    for _ in 0..4 {
        v = v * 16 + chars.next()?.to_digit(16)?;
    }
    Some(v)
}

/// Decode a PHP string body. Single-quoted strings escape only \\ and \';
/// every other backslash is literal (PHP semantics). Double-quoted strings
/// also get \" \$ \n \r \t \v \e \f; hex/octal/\u{...} escapes are rare in
/// translation keys and keep their backslash.
fn php_unescape(raw: &str, quote: u8) -> String {
    if !raw.contains('\\') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let next = chars.next();
        let decoded = match (quote, next) {
            (b'\'', Some('\'')) => Some('\''),
            (_, Some('\\')) => Some('\\'),
            (b'"', Some('"')) => Some('"'),
            (b'"', Some('$')) => Some('$'),
            (b'"', Some('n')) => Some('\n'),
            (b'"', Some('r')) => Some('\r'),
            (b'"', Some('t')) => Some('\t'),
            (b'"', Some('v')) => Some('\u{0B}'),
            (b'"', Some('e')) => Some('\u{1B}'),
            (b'"', Some('f')) => Some('\u{0C}'),
            _ => None,
        };
        match decoded {
            Some(d) => out.push(d),
            None => {
                out.push('\\');
                if let Some(o) = next {
                    out.push(o);
                }
            }
        }
    }
    out
}

/// `(nested.key, 0-based line)` for every `'key' =>` inside the array a
/// Laravel PHP lang file returns, flattened with dot notation down to
/// MAX_DEPTH. Brackets inside strings and comments are ignored; both line
/// (`//`, `#`) and block (`/* */`) comments are skipped. Mirrors the config
/// parser in env_index.rs, generalized past depth 2.
pub fn parse_lang_php(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut in_ret = false;
    let mut path: Vec<String> = Vec::new();
    let mut block_comment = false;
    for (i, line) in text.lines().enumerate() {
        let b = line.as_bytes();
        let mut j = 0;
        let ret_pos = if in_ret { None } else { line.find("return") };
        while j < b.len() {
            if !line.is_char_boundary(j) {
                j += 1;
                continue;
            }
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
                    while k < b.len() && b[k] != q {
                        if b[k] == b'\\' && k + 1 < b.len() {
                            k += 1;
                        }
                        k += 1;
                    }
                    let key = php_unescape(&line[j + 1..k.min(line.len())], q);
                    j = k + 1;
                    // `'key' =>` inside the returned array.
                    if in_ret
                        && depth >= 1
                        && depth <= MAX_DEPTH
                        && line[j.min(line.len())..]
                            .trim_start()
                            .starts_with("=>")
                    {
                        if !key.is_empty() && key.len() <= 100 {
                            path.truncate(depth - 1);
                            path.push(key);
                            out.push((path.join("."), i));
                        }
                    }
                }
                b'[' => {
                    if !in_ret && ret_pos.is_some_and(|r| r < j) {
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

/// `(key, 0-based line)` for each flat `"key": "value"` pair at depth 1 of a
/// JSON lang file. Escaped quotes inside keys are handled; nested objects
/// (unusual for lang JSON) are skipped.
pub fn parse_lang_json(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    for (i, line) in text.lines().enumerate() {
        let b = line.as_bytes();
        let mut j = 0;
        while j < b.len() {
            if !line.is_char_boundary(j) {
                j += 1;
                continue;
            }
            match b[j] {
                b'{' | b'[' => {
                    depth += 1;
                    j += 1;
                }
                b'}' | b']' => {
                    depth = depth.saturating_sub(1);
                    j += 1;
                }
                b'"' => {
                    let mut k = j + 1;
                    while k < b.len() && b[k] != b'"' {
                        if b[k] == b'\\' && k + 1 < b.len() {
                            k += 1;
                        }
                        k += 1;
                    }
                    let key = json_unescape(&line[j + 1..k.min(line.len())]);
                    j = k + 1;
                    if depth == 1
                        && !key.is_empty()
                        && key.len() <= 200
                        && line[j.min(line.len())..].trim_start().starts_with(':')
                    {
                        out.push((key, i));
                    }
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
    let lang = root.join("resources").join("lang");
    let Ok(rd) = std::fs::read_dir(&lang) else {
        return out;
    };
    let mut names: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            std::fs::symlink_metadata(p)
                .map(|m| m.is_dir() || m.is_file())
                .unwrap_or(false)
        })
        .collect();
    names.sort();
    let mut locales = 0usize;
    let mut files = 0usize;
    for entry in names {
        if out.len() >= MAX_ENTRIES || files >= MAX_FILES {
            break;
        }
        let name = entry
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let is_dir = entry.is_dir();
        if is_dir {
            locales += 1;
            if locales > MAX_LOCALES {
                break;
            }
            // Locale directory: each *.php group file.
            let Ok(files_rd) = std::fs::read_dir(&entry) else { continue };
            let mut php: Vec<PathBuf> = files_rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "php"))
                .collect();
            php.sort();
            for file in php {
                if out.len() >= MAX_ENTRIES || files >= MAX_FILES {
                    break;
                }
                files += 1;
                let Some(text) = read(&file) else { continue };
                let group = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if group.is_empty() {
                    continue;
                }
                let rel = file.strip_prefix(root).unwrap_or(&file).to_path_buf();
                for (key, line) in parse_lang_php(&text) {
                    if out.len() >= MAX_ENTRIES {
                        break;
                    }
                    let dotted = format!("{group}.{key}");
                    if seen.insert(dotted.clone()) {
                        out.push(RouteTarget {
                            label: format!("lang  {dotted}  ({})", rel.display()),
                            path: file.clone(),
                            line,
                        });
                    }
                }
            }
        } else if name.ends_with(".json") {
            let Some(text) = read(&entry) else { continue };
            for (key, line) in parse_lang_json(&text) {
                if out.len() >= MAX_ENTRIES {
                    break;
                }
                if seen.insert(key.clone()) {
                    out.push(RouteTarget {
                        label: format!("lang  {key}  ({name})"),
                        path: entry.clone(),
                        line,
                    });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_flattens_nested_keys_and_ignores_comments() {
        let text = r#"<?php
return [
    'failed' => 'These credentials do not match.',
    // 'hidden' => 'commented out',
    /* 'also_hidden' => 'x', */
    'throttle' => [
        'seconds' => 'Too many. :seconds',
        'nested' => [
            'deep' => 'valué',
        ],
    ],
];
"#;
        let keys = parse_lang_php(text);
        let names: Vec<&str> = keys.iter().map(|(k, _)| k.as_str()).collect();
        assert!(names.contains(&"failed"));
        assert!(names.contains(&"throttle"));
        assert!(names.contains(&"throttle.seconds"));
        assert!(names.contains(&"throttle.nested.deep"));
        assert!(!names.iter().any(|k| k.contains("hidden")));
        let seconds = keys.iter().find(|(k, _)| k == "throttle.seconds").unwrap();
        assert_eq!(seconds.1, 6);
    }

    #[test]
    fn php_ignores_brackets_and_arrows_in_strings() {
        let text = "<?php\nreturn [\n    'tricky' => 'a => b [not a key]',\n    'real' => 'v',\n];\n";
        let keys = parse_lang_php(text);
        let names: Vec<&str> = keys.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, vec!["tricky", "real"]);
    }

    #[test]
    fn php_never_panics_on_messy_input() {
        assert!(parse_lang_php("").is_empty());
        assert!(parse_lang_php("no array here").is_empty());
        let _ = parse_lang_php("<?php return ['a' => [");
        let _ = parse_lang_php("<?php return ['日本' => 'x', ' Escaped\\'quote' => 1];");
    }

    #[test]
    fn json_flat_keys_with_escapes_and_unicode() {
        let text = "{\n  \"Welcome back\": \"ようこそ\",\n  \"He said \\\"hi\\\"\": \"x\",\n  \"nested\": {\"skip\": \"me\"}\n}\n";
        let keys = parse_lang_json(text);
        let names: Vec<&str> = keys.iter().map(|(k, _)| k.as_str()).collect();
        assert!(names.contains(&"Welcome back"));
        // File bytes are He said \"hi\"; json_decode turns them into
        // He said "hi", and that decoded key is what __() expects.
        assert!(names.contains(&"He said \"hi\""));
        assert!(!names.contains(&"skip"));
        assert_eq!(keys[0].1, 1);
    }

    #[test]
    fn json_decodes_all_escape_kinds() {
        let text = "{\n  \"a\\\\b\": 1,\n  \"line\\nbreak\": 2,\n  \"caf\\u00e9\": 3,\n  \"smile \\uD83D\\uDE00\": 4\n}\n";
        let keys = parse_lang_json(text);
        let names: Vec<&str> = keys.iter().map(|(k, _)| k.as_str()).collect();
        assert!(names.contains(&"a\\b"));
        assert!(names.contains(&"line\nbreak"));
        assert!(names.contains(&"caf\u{e9}"));
        assert!(names.contains(&"smile \u{1F600}"));
    }

    #[test]
    fn php_decodes_string_escapes() {
        let text = "<?php\nreturn [\n    'it\\'s' => 1,\n    'a\\\\b' => 2,\n    'literal\\nslash' => 3,\n    \"say \\\"hi\\\"\" => 4,\n    \"cost \\$5\" => 5,\n];\n";
        let keys = parse_lang_php(text);
        let names: Vec<&str> = keys.iter().map(|(k, _)| k.as_str()).collect();
        assert!(names.contains(&"it's"));
        assert!(names.contains(&"a\\b"));
        // Single-quoted PHP has no \n escape: the backslash is literal.
        assert!(names.contains(&"literal\\nslash"));
        assert!(names.contains(&"say \"hi\""));
        assert!(names.contains(&"cost $5"));
    }

    #[test]
    fn json_never_panics_on_messy_input() {
        assert!(parse_lang_json("").is_empty());
        assert!(parse_lang_json("not json").is_empty());
        let _ = parse_lang_json("{\"a\": \"unclosed");
    }
}
