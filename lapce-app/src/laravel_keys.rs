//! Go to the Laravel config or translation key under the cursor:
//! `config('app.name')` opens `config/app.php` at the `'name' =>` line;
//! `__('messages.welcome')` / `trans(...)` / `@lang(...)` opens
//! `lang/<locale>/messages.php` (or `resources/lang/...`) at the key, and a
//! plain-text key such as `__('Welcome back')` opens the `lang/<locale>.json`
//! line. Textual only; `en` is tried before other locales; files up to 1 MB.
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_LOCALES: usize = 40;

const CONFIG_CALLERS: &[&str] = &[
    "config", "Config::get", "Config::has", "Config::string", "Config::integer",
    "Config::boolean", "Config::array", "Config::float",
];
const LANG_CALLERS: &[&str] = &[
    "__", "trans", "trans_choice", "@lang", "Lang::get", "Lang::has", "Lang::choice",
];

#[derive(Debug, PartialEq, Eq)]
enum Key {
    Config(String),
    Lang(String),
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn string_at(line: &str, col: usize) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\'' || bytes[i] == b'"' {
            let q = bytes[i] as char;
            let start = i + 1;
            let end = start + line[start..].find(q)?;
            if col >= start && col <= end {
                return Some((&line[start..end], &line[..i]));
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    None
}

/// True when `before` ends with `caller(` and the caller is a whole word.
fn first_arg_of(before: &str, callers: &[&str]) -> bool {
    let t = before.trim_end();
    let Some(t) = t.strip_suffix('(') else { return false };
    let t = t.trim_end();
    callers.iter().any(|c| {
        t.strip_suffix(c).is_some_and(|rest| {
            rest.chars().next_back().is_none_or(|p| !is_ident(p) && p != '$' && p != '>' && p != ':')
                || (c.starts_with('@') && !rest.chars().next_back().is_some_and(is_ident))
        })
    })
}

fn classify(line: &str, col: usize) -> Option<Key> {
    let (s, before) = string_at(line, col.min(line.len()))?;
    if s.is_empty() || s.len() > 200 || s.contains("::") {
        return None;
    }
    if first_arg_of(before, CONFIG_CALLERS) {
        let ok = s.chars().all(|c| is_ident(c) || matches!(c, '.' | '-'));
        return ok.then(|| Key::Config(s.to_string()));
    }
    if first_arg_of(before, LANG_CALLERS) {
        return Some(Key::Lang(s.to_string()));
    }
    None
}

/// Line of the deepest nested key among `segs`, searched in order.
fn key_line(text: &str, segs: &[&str]) -> usize {
    let lines: Vec<&str> = text.lines().collect();
    let mut cur = 0;
    let mut found = 0;
    for seg in segs {
        let hit = lines.iter().enumerate().skip(cur).find(|(_, l)| {
            let l = l.trim_start();
            [format!("'{seg}'"), format!("\"{seg}\"")].iter().any(|k| {
                l.strip_prefix(k.as_str()).is_some_and(|r| r.trim_start().starts_with("=>"))
            })
        });
        match hit {
            Some((i, _)) => {
                cur = i;
                found = i;
            }
            None => break,
        }
    }
    found
}

fn read(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    (meta.is_file() && meta.len() <= MAX_FILE_BYTES)
        .then(|| std::fs::read_to_string(path).ok())
        .flatten()
}

fn locales(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .take(MAX_LOCALES)
        .collect();
    v.sort_by_key(|l| (l != "en", l.clone()));
    v
}

fn lang_roots(root: &Path) -> [PathBuf; 2] {
    [root.join("lang"), root.join("resources").join("lang")]
}

fn resolve_config(root: &Path, key: &str) -> Option<(PathBuf, usize)> {
    let mut segs = key.split('.');
    let file = segs.next()?;
    let path = root.join("config").join(format!("{file}.php"));
    let text = read(&path)?;
    let rest: Vec<&str> = segs.collect();
    Some((path, key_line(&text, &rest)))
}

fn resolve_lang(root: &Path, key: &str) -> Option<(PathBuf, usize)> {
    let phpish = key.contains('.')
        && !key.contains(char::is_whitespace)
        && key.chars().all(|c| is_ident(c) || matches!(c, '.' | '-'));
    for dir in lang_roots(root) {
        if phpish {
            let mut parts = key.split('.');
            let file = parts.next()?;
            let rest: Vec<&str> = parts.collect();
            for loc in locales(&dir) {
                let path = dir.join(&loc).join(format!("{file}.php"));
                if let Some(text) = read(&path) {
                    return Some((path, key_line(&text, &rest)));
                }
            }
        } else {
            let mut names = vec!["en".to_string()];
            names.extend(locales_from_json(&dir));
            for loc in names {
                let path = dir.join(format!("{loc}.json"));
                if let Some(text) = read(&path) {
                    let needle = format!("\"{}\"", key.replace('\\', "\\\\").replace('"', "\\\""));
                    if let Some(i) = text.lines().position(|l| {
                        l.trim_start().strip_prefix(needle.as_str()).is_some_and(|r| r.trim_start().starts_with(':'))
                    }) {
                        return Some((path, i));
                    }
                }
            }
        }
    }
    None
}

fn locales_from_json(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".json").map(str::to_string))
        .filter(|n| n != "en")
        .take(MAX_LOCALES)
        .collect();
    v.sort();
    v
}

/// `col` clamped to the line and moved back onto a char boundary, so slicing
/// can never panic on multibyte text.
fn clamp_col(line: &str, col: usize) -> usize {
    let mut c = col.min(line.len());
    while !line.is_char_boundary(c) {
        c -= 1;
    }
    c
}

/// Target of the config/translation key at byte `col` of `line`.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let col = clamp_col(line, col);
    match classify(line, col)? {
        Key::Config(k) => resolve_config(root, &k),
        Key::Lang(k) => resolve_lang(root, &k),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    fn put(root: &Path, rel: &str, text: &str) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }

    #[test]
    fn classifies_calls() {
        let c = |l: &str, n: &str| classify(l, at(l, n));
        assert_eq!(c("$n = config('app.name');", "app"), Some(Key::Config("app.name".into())));
        assert_eq!(c("Config::get(\"mail.from.address\")", "mail"), Some(Key::Config("mail.from.address".into())));
        assert_eq!(c("{{ __('messages.welcome') }}", "messages"), Some(Key::Lang("messages.welcome".into())));
        assert_eq!(c("@lang('auth.failed')", "auth"), Some(Key::Lang("auth.failed".into())));
        assert_eq!(c("trans_choice('a.b', 2)", "a.b"), Some(Key::Lang("a.b".into())));
        assert_eq!(c("__('Welcome back')", "Welcome"), Some(Key::Lang("Welcome back".into())));
        // Default values, other calls, methods and variables with the same tail.
        assert_eq!(c("config('app.name', 'Laravel')", "Laravel"), None);
        assert_eq!(c("$this->config('app.name')", "app"), None);
        assert_eq!(c("myconfig('app.name')", "app"), None);
        assert_eq!(c("route('app.name')", "app"), None);
        assert_eq!(c("__('vendor::file.key')", "file"), None);
    }

    #[test]
    fn config_key_lines() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let p = put(r, "config/mail.php", "<?php\nreturn [\n  'default' => 'smtp',\n  'from' => [\n    'address' => 'a',\n    'name' => 'n',\n  ],\n  'name' => 'wrong',\n];\n");
        assert_eq!(resolve_config(r, "mail.default"), Some((p.clone(), 2)));
        assert_eq!(resolve_config(r, "mail.from.name"), Some((p.clone(), 5)));
        assert_eq!(resolve_config(r, "mail.from.address"), Some((p.clone(), 4)));
        // Unknown leaf: the deepest key found, unknown file: nothing.
        assert_eq!(resolve_config(r, "mail.from.nope"), Some((p.clone(), 3)));
        assert_eq!(resolve_config(r, "mail"), Some((p, 0)));
        assert_eq!(resolve_config(r, "nope.x"), None);
    }

    #[test]
    fn lang_php_and_json() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let en = put(r, "lang/en/messages.php", "<?php\nreturn [\n  'welcome' => 'Hi',\n  'bye' => 'Bye',\n];\n");
        put(r, "lang/de/messages.php", "<?php\nreturn [\n  'bye' => 'Tschuess',\n];\n");
        let json = put(r, "lang/en.json", "{\n  \"Sign in\": \"Sign in\",\n  \"Welcome back\": \"Hi\"\n}\n");
        assert_eq!(resolve_lang(r, "messages.bye"), Some((en.clone(), 3)));
        assert_eq!(resolve_lang(r, "Welcome back"), Some((json, 2)));
        assert_eq!(resolve_lang(r, "Missing text"), None);
        // Older layout and a locale other than en.
        let d2 = tempfile::tempdir().unwrap();
        let de = put(d2.path(), "resources/lang/de/auth.php", "<?php\nreturn [\n 'failed' => 'x',\n];\n");
        assert_eq!(resolve_lang(d2.path(), "auth.failed"), Some((de, 2)));
        let l = "__('messages.welcome')";
        assert_eq!(resolve(r, l, at(l, "messages")), Some((en, 2)));
    }
}
