//! Go to the PHP class named in a config file: a fully qualified class name
//! under the cursor in YAML, XML or Twig (`App\Service\Mailer`,
//! `@App\Service\Mailer`, `App\Controller\Home::index`, a service `class:`,
//! `resource: '../src/'` is ignored) opens the class declaration, mapped
//! through the PSR-4 entries of `composer.json` (`autoload` and
//! `autoload-dev`; `App\` => `src/` when none is given). `Class::method`
//! opens the method. Doubled backslashes (XML/JSON/Twig strings) are read as
//! one. Textual only; PHP files themselves are left to the language server.
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_PREFIXES: usize = 60;

fn is_class_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '\\')
}

/// `(class, optional method)` at byte `col` of `line`.
fn class_at(line: &str, col: usize) -> Option<(String, Option<String>)> {
    let col = col.min(line.len());
    if !line.is_char_boundary(col) {
        return None;
    }
    let start = line[..col]
        .rfind(|c: char| !is_class_char(c))
        .map(|i| i + line[i..].chars().next().map_or(1, char::len_utf8))
        .unwrap_or(0);
    let end = line[col..]
        .find(|c: char| !is_class_char(c))
        .map(|i| col + i)
        .unwrap_or(line.len());
    if start >= end || end - start > 300 {
        return None;
    }
    let raw = line[start..end].replace("\\\\", "\\");
    let class = raw.trim_matches('\\');
    // A qualified name: at least one namespace separator and an uppercase start.
    if !class.contains('\\')
        || class.split('\\').any(|s| s.is_empty() || s.starts_with(|c: char| c.is_ascii_digit()))
        || !class.starts_with(|c: char| c.is_ascii_uppercase())
    {
        return None;
    }
    let method = line[end..]
        .strip_prefix("::")
        .map(|r| r.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect::<String>())
        .filter(|m| !m.is_empty() && m != "class");
    Some((class.to_string(), method))
}

/// PSR-4 `(prefix, dir)` pairs, longest prefix first.
pub(crate) fn psr4(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    let doc = std::fs::metadata(root.join("composer.json"))
        .ok()
        .filter(|m| m.is_file() && m.len() <= MAX_FILE_BYTES)
        .and_then(|_| std::fs::read_to_string(root.join("composer.json")).ok())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    if let Some(doc) = doc {
        for section in ["autoload", "autoload-dev"] {
            let Some(map) = doc.get(section).and_then(|a| a.get("psr-4")).and_then(|p| p.as_object()) else {
                continue;
            };
            for (prefix, dirs) in map.iter().take(MAX_PREFIXES) {
                let list: Vec<&str> = match dirs {
                    serde_json::Value::String(s) => vec![s.as_str()],
                    serde_json::Value::Array(a) => a.iter().filter_map(|v| v.as_str()).collect(),
                    _ => vec![],
                };
                for d in list {
                    out.push((prefix.clone(), root.join(d.trim_start_matches("./"))));
                }
            }
        }
    }
    if out.is_empty() {
        out.push(("App\\".to_string(), root.join("src")));
    }
    out.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
    out
}

/// The file declaring `class` through PSR-4.
pub fn class_file(root: &Path, class: &str) -> Option<PathBuf> {
    let full = format!("{class}");
    for (prefix, dir) in psr4(root) {
        if let Some(rest) = full.strip_prefix(prefix.as_str()) {
            if rest.is_empty() || (!prefix.ends_with('\\') && !rest.starts_with('\\')) {
                continue;
            }
            let rel = rest.trim_start_matches('\\').replace('\\', "/");
            if rel.split('/').any(|s| s == ".." || s.is_empty()) {
                continue;
            }
            let p = dir.join(format!("{rel}.php"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn line_of(text: &str, needles: &[String]) -> Option<usize> {
    text.lines().position(|l| {
        let t = l.trim_start();
        needles.iter().any(|n| {
            t.match_indices(n.as_str()).any(|(i, _)| {
                // `class Name` as a declaration: Name ends at a word boundary.
                let after = t[i + n.len()..].chars().next();
                after.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
                    && t[..i].split_whitespace().all(|w| {
                        matches!(w, "final" | "abstract" | "readonly" | "public" | "protected"
                            | "private" | "static" | "#[" )
                            || w.starts_with("#[")
                            || w.is_empty()
                    })
            })
        })
    })
}

/// File and zero-based line for the class (or method) under the cursor.
pub fn resolve(root: &Path, file: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let ext = file.extension()?.to_str()?;
    if !matches!(ext, "yaml" | "yml" | "xml" | "twig" | "neon") {
        return None;
    }
    let (class, method) = class_at(line, col)?;
    let path = class_file(root, &class)?;
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() > MAX_FILE_BYTES {
        return Some((path, 0));
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let short = class.rsplit('\\').next()?;
    if let Some(m) = method {
        if let Some(i) = line_of(&text, &[format!("function {m}")]) {
            return Some((path, i));
        }
    }
    let decl: Vec<String> = ["class", "interface", "trait", "enum"]
        .iter()
        .map(|k| format!("{k} {short}"))
        .collect();
    let i = line_of(&text, &decl).unwrap_or(0);
    Some((path, i))
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
    fn extracts_class_names() {
        let c = |l: &str, n: &str| class_at(l, at(l, n));
        assert_eq!(c("    App\\Service\\Mailer:", "Mailer"), Some(("App\\Service\\Mailer".into(), None)));
        assert_eq!(c("    arguments: ['@App\\Service\\Mailer']", "Service"), Some(("App\\Service\\Mailer".into(), None)));
        assert_eq!(c("<service id=\"App\\\\Foo\\\\Bar\"/>", "Foo"), Some(("App\\Foo\\Bar".into(), None)));
        assert_eq!(c("controller: App\\Controller\\Home::index", "Home"), Some(("App\\Controller\\Home".into(), Some("index".into()))));
        assert_eq!(c("class: App\\X::class", "App"), Some(("App\\X".into(), None)));
        // Unqualified names, paths and lowercase starts are not classes.
        assert_eq!(c("    resource: '../src/'", "src"), None);
        assert_eq!(c("    path: /blog/{slug}", "blog"), None);
        assert_eq!(c("Mailer", "Mailer"), None);
        assert_eq!(c("app\\x", "app"), None);
    }

    #[test]
    fn resolves_through_psr4_with_longest_prefix() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "composer.json", r#"{"autoload":{"psr-4":{"App\\":"src/","App\\Legacy\\":["legacy/"]}},"autoload-dev":{"psr-4":{"App\\Tests\\":"tests/"}}}"#);
        let mailer = put(r, "src/Service/Mailer.php", "<?php\nnamespace App\\Service;\n\n#[Foo]\nfinal class Mailer\n{\n    public function send() {}\n}\n");
        let old = put(r, "legacy/Thing.php", "<?php\ninterface Thing {}\n");
        let t = put(r, "tests/FooTest.php", "<?php\nclass FooTest {}\n");
        let y = r.join("config/services.yaml");
        let l = "    App\\Service\\Mailer: ~";
        assert_eq!(resolve(r, &y, l, at(l, "Mailer")), Some((mailer.clone(), 4)));
        let l = "controller: App\\Service\\Mailer::send";
        assert_eq!(resolve(r, &y, l, at(l, "Mailer")), Some((mailer, 6)));
        let l = "- '@App\\Legacy\\Thing'";
        assert_eq!(resolve(r, &y, l, at(l, "Thing")), Some((old, 1)));
        let l = "App\\Tests\\FooTest";
        assert_eq!(resolve(r, &y, l, 3), Some((t, 1)));
        // Missing class, and PHP files are left to the language server.
        let l = "App\\Missing\\Nope";
        assert_eq!(resolve(r, &y, l, 6), None);
        let l = "    App\\Service\\Mailer: ~";
        assert_eq!(resolve(r, &r.join("a.php"), l, at(l, "Mailer")), None);
    }

    #[test]
    fn defaults_to_app_src_and_rejects_escapes() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let f = put(r, "src/Kernel.php", "<?php\nclass Kernel {}\n");
        let y = r.join("services.yaml");
        assert_eq!(resolve(r, &y, "App\\Kernel", 5), Some((f, 1)));
        assert_eq!(resolve(r, &y, "App\\..\\Kernel", 5), None);
        assert_eq!(class_file(r, "App\\"), None);
        assert_eq!(class_file(r, "Apple\\Kernel"), None);
    }
}
