//! Go to the `.env` line that defines the variable under the cursor:
//! `env('APP_NAME')`, `getenv('X')`, `$_ENV['X']`, `$_SERVER['X']`,
//! `%env(DATABASE_URL)%` / `%env(int:PORT)%` (Symfony), `process.env.X`,
//! `import.meta.env.VITE_X`, and `${X}` inside `.env` files.
//!
//! Files tried in order: `.env`, `.env.local`, `.env.example`, `.env.dist`
//! (the first that defines the key wins), each read up to 1 MB. Nothing is
//! executed. `None` means "not an env reference or not defined anywhere".
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const FILES: &[&str] = &[".env", ".env.local", ".env.example", ".env.dist"];

const PREFIXES: &[&str] = &[
    "env('", "env(\"", "getenv('", "getenv(\"", "_ENV['", "_ENV[\"", "_SERVER['",
    "_SERVER[\"", "process.env.", "import.meta.env.", "${", "%env(",
];

fn is_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The variable name at byte `col` of `line`, when it is in an env reference.
pub fn key_at(line: &str, col: usize) -> Option<&str> {
    let col = col.min(line.len());
    if !line.is_char_boundary(col) {
        return None;
    }
    let start = line[..col]
        .rfind(|c: char| !is_key_char(c))
        .map(|i| i + line[i..].chars().next().map_or(1, char::len_utf8))
        .unwrap_or(0);
    let end = line[col..]
        .find(|c: char| !is_key_char(c))
        .map(|i| col + i)
        .unwrap_or(line.len());
    if start >= end || end - start > 100 {
        return None;
    }
    let before = &line[..start];
    // Symfony processors: `%env(int:PORT)%`, `%env(json:file:X)%`.
    let before_proc = before
        .rfind("%env(")
        .filter(|i| before[i + 5..].chars().all(|c| is_key_char(c) || c == ':'))
        .map(|_| "%env(");
    let hit = before_proc.is_some() || PREFIXES.iter().any(|p| before.ends_with(p));
    hit.then(|| &line[start..end])
}

/// `(file, zero-based line)` of the first definition of `key`.
pub fn find_definition(root: &Path, key: &str) -> Option<(PathBuf, usize)> {
    for name in FILES {
        let path = root.join(name);
        let Ok(meta) = std::fs::metadata(&path) else { continue };
        if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        for (i, l) in text.lines().enumerate() {
            let l = l.trim_start();
            let l = l.strip_prefix("export ").map(str::trim_start).unwrap_or(l);
            if let Some(rest) = l.strip_prefix(key) {
                if rest.trim_start().starts_with('=') {
                    return Some((path, i));
                }
            }
        }
    }
    None
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

/// Where the env reference under the cursor is defined.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let col = clamp_col(line, col);
    find_definition(root, key_at(line, col)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    #[test]
    fn recognises_env_references() {
        for (l, n, k) in [
            ("'name' => env('APP_NAME', 'Laravel'),", "APP_NAME", "APP_NAME"),
            ("$x = getenv(\"DB_HOST\");", "DB_HOST", "DB_HOST"),
            ("$_ENV['MAIL_FROM'] ?? null", "MAIL_FROM", "MAIL_FROM"),
            ("url: '%env(resolve:DATABASE_URL)%'", "DATABASE_URL", "DATABASE_URL"),
            ("url: '%env(DATABASE_URL)%'", "DATABASE_URL", "DATABASE_URL"),
            ("const a = process.env.API_URL;", "API_URL", "API_URL"),
            ("fetch(import.meta.env.VITE_API)", "VITE_API", "VITE_API"),
            ("URL=${APP_URL}/x", "APP_URL", "APP_URL"),
        ] {
            assert_eq!(key_at(l, at(l, n)), Some(k), "{l}");
        }
        // Plain strings, other functions and the default-value argument.
        assert_eq!(key_at("route('APP_NAME')", 8), None);
        let l = "env('APP_NAME', 'Laravel')";
        assert_eq!(key_at(l, at(l, "Laravel")), None);
        assert_eq!(key_at("", 0), None);
        assert_eq!(key_at("env('é", 6), None);
    }

    #[test]
    fn finds_the_defining_line_across_env_files() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::write(r.join(".env.example"), "A=1\nAPP_KEY=\nONLY_EXAMPLE=x\n").unwrap();
        std::fs::write(r.join(".env"), "# c\nAPP_NAME=Acme\nexport  DB_HOST = h\nAPP_NAME_2=z\n").unwrap();
        assert_eq!(find_definition(r, "APP_NAME"), Some((r.join(".env"), 1)));
        assert_eq!(find_definition(r, "DB_HOST"), Some((r.join(".env"), 2)));
        // Falls through to .env.example; prefix names do not match.
        assert_eq!(find_definition(r, "APP_KEY"), Some((r.join(".env.example"), 1)));
        assert_eq!(find_definition(r, "APP_N"), None);
        assert_eq!(find_definition(r, "MISSING"), None);
        let l = "env('ONLY_EXAMPLE')";
        assert_eq!(resolve(r, l, at(l, "ONLY")), Some((r.join(".env.example"), 2)));
    }
}
