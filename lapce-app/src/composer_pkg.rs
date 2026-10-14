//! Go to a Composer package from `composer.json` / `composer.lock`: with the
//! cursor on a `"vendor/package"` key in `require`, `require-dev`, `suggest`
//! and the like (or on a `"name": "vendor/package"` value in `composer.lock`)
//! the package's own `composer.json` under `vendor/` opens, where its
//! autoload and requirements are. Nothing is run; platform requirements such
//! as `php` and `ext-json` and packages that are not installed are ignored.
use std::path::{Path, PathBuf};

fn is_name_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-')
}

/// `vendor/package` as a valid Composer package name.
pub fn valid_package(name: &str) -> bool {
    let Some((vendor, package)) = name.split_once('/') else {
        return false;
    };
    let ok = |s: &str| !s.is_empty() && s.chars().all(is_name_char) && s != "." && s != "..";
    ok(vendor) && ok(package)
}

/// The quoted string under `col` and the text before its opening quote.
fn string_at(line: &str, col: usize) -> Option<(&str, &str)> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            let start = i + 1;
            let mut end = start;
            while end < b.len() && b[end] != b'"' {
                end += if b[end] == b'\\' { 2 } else { 1 };
            }
            let end = end.min(b.len());
            if col >= start && col <= end {
                return Some((line.get(start..end)?, &line[..i]));
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    None
}

/// The package named under the cursor, or None.
pub fn package_at(file: &Path, line: &str, col: usize) -> Option<String> {
    let fname = file.file_name()?.to_str()?;
    if fname != "composer.json" && fname != "composer.lock" {
        return None;
    }
    let (s, before) = string_at(line, col)?;
    let before = before.trim();
    let is_key = before.is_empty() || before.ends_with(',') || before.ends_with('{');
    let is_name_value = before.ends_with(':') && before.trim_end_matches(':').trim_end().ends_with("\"name\"");
    let ok = match fname {
        "composer.json" => is_key && valid_package(s),
        _ => is_name_value && valid_package(s),
    };
    ok.then(|| s.to_string())
}

/// File (and line) of the package's own composer.json.
pub fn resolve(root: &Path, file: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let name = package_at(file, line, col)?;
    let target = root.join("vendor").join(&name).join("composer.json");
    std::fs::metadata(&target).ok().filter(|m| m.is_file())?;
    // Symlinked (path-repository) packages may point outside vendor/; only
    // follow the ones that stay inside it.
    let real = target.canonicalize().ok()?;
    let vendor = root.join("vendor").canonicalize().ok()?;
    if real.starts_with(&vendor) {
        Some((target, 0))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn project() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("vendor/laravel/framework");
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("composer.json"), "{}").unwrap();
        d
    }

    #[test]
    fn package_names() {
        assert!(valid_package("laravel/framework"));
        assert!(valid_package("a.b/c_d-e"));
        assert!(!valid_package("php"));
        assert!(!valid_package("ext-json"));
        assert!(!valid_package("../x"));
        assert!(!valid_package("a/../b"));
        assert!(!valid_package("Laravel/Framework"));
        assert!(!valid_package("a/b/c"));
        assert!(!valid_package("/b"));
        assert!(!valid_package("a/"));
    }

    #[test]
    fn composer_json_keys_only() {
        let f = Path::new("composer.json");
        let l = "        \"laravel/framework\": \"^11.0\",";
        assert_eq!(package_at(f, l, 14).as_deref(), Some("laravel/framework"));
        // the version string is not a package
        assert_eq!(package_at(f, l, 32), None);
        assert_eq!(package_at(f, "    \"name\": \"acme/app\",", 15), None);
        assert_eq!(package_at(f, "    \"php\": \"^8.2\",", 8), None);
        assert_eq!(package_at(Path::new("a.json"), l, 14), None);
    }

    #[test]
    fn composer_lock_names() {
        let f = Path::new("composer.lock");
        let l = "            \"name\": \"laravel/framework\",";
        assert_eq!(package_at(f, l, 24).as_deref(), Some("laravel/framework"));
        assert_eq!(package_at(f, "            \"type\": \"library\",", 24), None);
        assert_eq!(package_at(f, "            \"laravel/framework\": \"^1\"", 20), None);
    }

    #[test]
    fn resolves_installed_packages_only() {
        let d = project();
        let r = d.path();
        let f = r.join("composer.json");
        let l = "    \"laravel/framework\": \"^11\"";
        assert_eq!(resolve(r, &f, l, 8), Some((r.join("vendor/laravel/framework/composer.json"), 0)));
        assert_eq!(resolve(r, &f, "    \"acme/missing\": \"^1\"", 8), None);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_out_of_vendor_is_ignored() {
        let d = project();
        let r = d.path();
        fs::create_dir_all(r.join("elsewhere")).unwrap();
        fs::write(r.join("elsewhere/composer.json"), "{}").unwrap();
        fs::create_dir_all(r.join("vendor/acme")).unwrap();
        std::os::unix::fs::symlink(r.join("elsewhere"), r.join("vendor/acme/linked")).unwrap();
        assert_eq!(resolve(r, &r.join("composer.json"), "    \"acme/linked\": \"*\"", 8), None);
    }

    #[test]
    fn odd_lines_do_not_panic() {
        let f = Path::new("composer.json");
        for l in ["", "\"", "\"\\", "\"a/b", "é\"é/é\"", "\"\"", "{\"a/b\": 1}", &"\"".repeat(10_000)] {
            for c in [0, 1, 2, l.len() / 2, l.len(), l.len() + 5, usize::MAX] {
                let _ = package_at(f, l, c);
            }
        }
        assert_eq!(package_at(f, "{\"a/b\": 1}", 3).as_deref(), Some("a/b"));
    }
}
