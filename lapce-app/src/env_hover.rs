//! Hover an env reference - `env('APP_NAME')`, `getenv('DB_HOST')`,
//! `$_ENV['X']`, `%env(DATABASE_URL)%`, `process.env.X` - and see where the
//! variable is defined (`.env`, `.env.local`, `.env.example`, `.env.dist`),
//! whether the match is a commented-out line, and the code default from a
//! second argument. Values are never shown, matching the env palette index.

use std::path::Path;

const MAX_FILE: u64 = 1 << 20;

/// The second argument of `env('KEY', ...)` / `getenv('KEY', ...)` when the
/// first argument is exactly `key`. Quoted strings and bare scalars
/// (`true`, `false`, `null`, numbers); capped so a huge default is skipped.
fn default_arg(line: &str, key: &str) -> Option<String> {
    for func in ["env(", "getenv("] {
        for (fi, _) in line.match_indices(func) {
            let rest = &line[fi + func.len()..];
            let rest = rest.trim_start();
            let Some(q) = rest.chars().next().filter(|c| *c == '\'' || *c == '"')
            else {
                continue;
            };
            let Some(end) = rest[1..].find(q) else { continue };
            if &rest[1..1 + end] != key {
                continue;
            }
            // This call is the one; the default decides the result.
            let after = rest[1 + end + 1..].trim_start();
            let Some(after) = after.strip_prefix(',') else { return None };
            let v = after.trim_start();
            if let Some(q2) = v.chars().next().filter(|c| *c == '\'' || *c == '"') {
                let Some(e2) = v[1..].find(q2) else { return None };
                let d = &v[1..1 + e2];
                return (d.len() <= 80).then(|| d.to_string());
            }
            let stop = v.find([',', ')']).unwrap_or(v.len());
            let d = v[..stop].trim();
            return (!d.is_empty() && d.len() <= 40).then(|| d.to_string());
        }
    }
    None
}

/// Markdown hover for the env reference at byte `col` of `line`.
pub fn help(root: Option<&Path>, line: &str, col: usize) -> Option<String> {
    let root = root?;
    if line.len() > 4096 {
        return None;
    }
    let key = crate::env_refs::key_at(line, col)?.to_string();
    let default = default_arg(line, &key);
    let (where_, detail) = match crate::env_refs::find_definition(root, &key) {
        Some((path, idx)) => {
            // Re-read just enough to tell a live definition from a
            // commented-out one (find_definition falls back to `# KEY=`).
            let commented = std::fs::metadata(&path)
                .ok()
                .filter(|m| m.len() <= MAX_FILE)
                .and_then(|_| std::fs::read_to_string(&path).ok())
                .and_then(|t| t.lines().nth(idx).map(str::to_string))
                .is_some_and(|l| l.trim_start().starts_with('#'));
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let what = if commented {
                format!("commented out in `{rel}`", rel = rel.display())
            } else {
                format!("set in `{rel}`", rel = rel.display())
            };
            (what, format!("\n\n_{}:{}_", rel.display(), idx + 1))
        }
        None => (
            "not defined in any .env file".to_string(),
            String::new(),
        ),
    };
    let dflt = default
        .map(|d| format!("\n\nCode default: `{d}`."))
        .unwrap_or_default();
    Some(format!("**`{key}`** - {where_}{detail}{dflt}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: &str, needle: &str) -> usize {
        line.find(needle).unwrap() + 1
    }

    #[test]
    fn env_references_are_explained() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::write(r.join(".env"), "APP_NAME=Acme\nDB_HOST=db\n").unwrap();
        std::fs::write(r.join(".env.example"), "# MAIL_FROM=hello@example.com\n").unwrap();
        let root = Some(r);

        let l = "'name' => env('APP_NAME', 'Laravel'),";
        let md = help(root, l, at(l, "APP_NAME")).unwrap();
        assert!(md.contains("**`APP_NAME`** - set in `.env`\n\n_.env:1_"), "{md}");
        assert!(md.contains("Code default: `Laravel`."), "{md}");

        // A commented-out definition is reported as such.
        let l = "$x = env('MAIL_FROM');";
        let md = help(root, l, at(l, "MAIL_FROM")).unwrap();
        assert!(md.contains("commented out in `.env.example`\n\n_.env.example:1_"), "{md}");

        // Unknown variable, no second argument.
        let l = "$x = env('NOPE_MISSING');";
        let md = help(root, l, at(l, "NOPE_MISSING")).unwrap();
        assert!(md.contains("not defined in any .env file"), "{md}");
        assert!(!md.contains("Code default"), "{md}");

        // Bare scalar default; not a reference: nothing.
        let l = "'debug' => env('APP_DEBUG', false),";
        let md = help(root, l, at(l, "APP_DEBUG")).unwrap();
        assert!(md.contains("Code default: `false`."), "{md}");
        let l = "'APP_NAME' => 1,";
        assert_eq!(help(root, l, at(l, "APP_NAME")), None);
        assert_eq!(help(None, "env('APP_NAME')", 6), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        let root = Some(d.path());
        for l in ["env('", "env('A', '", "env()", "'_'", "env('é', 'ü')", "env('日本')"] {
            for c in 0..=l.len() + 2 {
                let _ = help(root, l, c);
            }
        }
    }
}
