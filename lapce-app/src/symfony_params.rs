//! Go to a Symfony container parameter from its use: `%app.admin_email%` in
//! YAML, XML or PHP (`#[Autowire('%app.admin_email%')]`), and the string in
//! `$this->getParameter('app.admin_email')` / `hasParameter(...)` /
//! `#[Autowire(param: '...')]`. It opens the `app.admin_email:` line under
//! `parameters:` in `config/services*.yaml` or `config/packages/**.yaml`, or the
//! `<parameter key="...">` line in `config/services*.xml`.
//!
//! Textual only; nothing is executed. Only keys under a top-level
//! `parameters:` block count, so a service id of the same name is not matched.
//! Built-in parameters (`kernel.project_dir`), `%env(...)%` and parameters set
//! from PHP are not found. Bounded: 80 config files, 1 MB per file.
use std::path::{Path, PathBuf};

const MAX_FILES: usize = 80;
const MAX_FILE_BYTES: u64 = 1_000_000;

fn is_param_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
}

/// The parameter name under byte `col`.
fn param_at(line: &str, col: usize) -> Option<&str> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    // `%name%`
    if let Some(left) = line[..col].rfind('%') {
        if let Some(r) = line[col..].find('%') {
            let name = &line[left + 1..col + r];
            if !name.is_empty() && name.len() <= 100 && name.chars().all(is_param_char) {
                return Some(name);
            }
        }
    }
    // getParameter('name'), hasParameter('name'), Autowire(param: 'name')
    let i = line[..col].rfind(['\'', '"'])?;
    let q = line[i..].chars().next()?;
    let end = col + line[col..].find(q)?;
    let name = &line[i + 1..end];
    let before = line[..i].trim_end();
    let ok = before.ends_with("getParameter(")
        || before.ends_with("hasParameter(")
        || before.ends_with("param:");
    (ok && !name.is_empty() && name.len() <= 100 && name.chars().all(is_param_char)).then_some(name)
}

fn config_files(root: &Path) -> Vec<PathBuf> {
    let cfg = root.join("config");
    let mut out = Vec::new();
    let mut dirs = vec![cfg.clone(), cfg.join("packages")];
    if let Ok(rd) = std::fs::read_dir(cfg.join("packages")) {
        for e in rd.flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                dirs.push(e.path());
            }
        }
    }
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut files: Vec<PathBuf> = rd
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| e.path())
            .filter(|p| {
                p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    let services = d == cfg && !n.starts_with("services");
                    !services && (n.ends_with(".yaml") || n.ends_with(".yml") || n.ends_with(".xml"))
                })
            })
            .collect();
        files.sort();
        out.extend(files);
        if out.len() >= MAX_FILES {
            break;
        }
    }
    out.truncate(MAX_FILES);
    out
}

fn yaml_key_is(trimmed: &str, name: &str) -> bool {
    let t = trimmed.trim_start_matches(['\'', '"']);
    t.strip_prefix(name)
        .map(|r| r.trim_start_matches(['\'', '"']))
        .is_some_and(|r| r.trim_start().starts_with(':'))
}

fn find_line(text: &str, name: &str, xml: bool) -> Option<usize> {
    if xml {
        return text.lines().position(|l| {
            l.contains("<parameter")
                && (l.contains(&format!("key=\"{name}\"")) || l.contains(&format!("key='{name}'")))
        });
    }
    let mut in_params = false;
    for (i, l) in text.lines().enumerate() {
        if l.trim().is_empty() || l.trim_start().starts_with('#') {
            continue;
        }
        if !l.starts_with([' ', '\t']) {
            in_params = l.trim_end().strip_prefix("parameters:").is_some_and(|r| {
                let r = r.trim();
                r.is_empty() || r.starts_with('#')
            });
            continue;
        }
        if in_params && yaml_key_is(l.trim_start(), name) {
            return Some(i);
        }
    }
    None
}

/// Config file and 0-based line of the parameter for the name under `col`.
pub fn resolve(root: &Path, line: &str, col: usize) -> Option<(PathBuf, usize)> {
    let name = param_at(line, col)?;
    for p in config_files(root) {
        let Ok(meta) = std::fs::metadata(&p) else { continue };
        if meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let xml = p.extension().is_some_and(|e| e == "xml");
        if let Some(i) = find_line(&text, name, xml) {
            return Some((p, i));
        }
    }
    None
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
    fn parameters_open_their_declaration() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        let svc = put(r, "config/services.yaml", "# c\nparameters:\n    app.admin_email: 'a@b.c'\n    \"app.quoted\": 1\n\nservices:\n    app.service_only: ~\n    _defaults:\n        autowire: true\n");
        let pkg = put(r, "config/packages/twig.yaml", "twig:\n    x: 1\nparameters:   # more\n  twig.extra: yes\n");
        let xml = put(r, "config/services_extra.xml", "<container>\n  <parameters>\n    <parameter key=\"app.xml_param\">1</parameter>\n  </parameters>\n</container>\n");
        let go = |l: &str, n: &str| resolve(r, l, at(l, n));
        let l = "        arguments: ['%app.admin_email%']";
        assert_eq!(go(l, "admin_email"), Some((svc.clone(), 2)));
        let l = "$e = $this->getParameter('app.quoted');";
        assert_eq!(go(l, "app.quoted"), Some((svc.clone(), 3)));
        let l = "#[Autowire(param: 'twig.extra')]";
        assert_eq!(go(l, "twig.extra"), Some((pkg, 3)));
        let l = "<argument>%app.xml_param%</argument>";
        assert_eq!(go(l, "xml_param"), Some((xml, 2)));
        // A service id is not a parameter; env and built-ins are not found.
        let l = "x: '%app.service_only%'";
        assert_eq!(go(l, "service_only"), None);
        let l = "x: '%env(APP_X)%'";
        assert_eq!(go(l, "APP_X"), None);
        let l = "x: '%kernel.project_dir%'";
        assert_eq!(go(l, "project_dir"), None);
        let l = "$p = $this->get('app.admin_email');";
        assert_eq!(go(l, "admin_email"), None);
    }

    #[test]
    fn odd_input_does_not_panic() {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "config/services.yaml", "parameters:\n  é: 1\n");
        for l in ["%", "%%", "%é%", "'%é.ü%'", "getParameter('é", "param: '日本'", "", "%a% x %b%", "😀%😀"] {
            for c in 0..=l.len() + 2 {
                let _ = resolve(d.path(), l, c);
            }
        }
    }
}
