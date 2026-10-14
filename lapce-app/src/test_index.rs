//! Index for the "Go to Test" palette: test files and test cases under
//! `tests/` (Laravel, Symfony, WordPress, plain PHPUnit and Pest).
//!
//! - a file ending in `Test.php` or `.test.php`... is listed as `test-file`;
//! - PHPUnit methods named `test*`, or preceded by `#[Test]` / `@test`, are
//!   listed as `test  UserTest::test_can_login`;
//! - Pest `it('does x')` / `test('does x')` calls at line start are listed as
//!   `test  UserTest: it does x`.
//!
//! Static text scan, nothing is executed. Bounded: depth 6, 1500 files, 1 MB
//! per file, 5000 entries, symlinks skipped.
use std::path::{Path, PathBuf};

use crate::route_index::RouteTarget;

const MAX_FILE: u64 = 1 << 20;
const MAX_FILES: usize = 1500;
const MAX_DEPTH: usize = 6;
const MAX_ENTRIES: usize = 5000;

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_FILES {
        return;
    }
    if depth == 0 && std::fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink()) {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let p = e.path();
        if ft.is_dir() {
            if p.file_name().is_some_and(|n| n == "vendor" || n == "node_modules") {
                continue;
            }
            walk(&p, depth + 1, out);
        } else if ft.is_file() && p.extension().is_some_and(|x| x == "php") {
            out.push(p);
        }
        if out.len() >= MAX_FILES {
            return;
        }
    }
}

fn is_test_file(name: &str) -> bool {
    name.ends_with("Test.php") || name.ends_with("-test.php") || name.ends_with(".test.php") || name.ends_with("Spec.php")
}

fn ident(s: &str) -> Option<&str> {
    let end = s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(s.len());
    (end > 0).then(|| &s[..end])
}

/// A Pest description: `it('does x'` / `test("does x"` -> `it does x`.
fn pest_title(line: &str) -> Option<String> {
    let (kw, rest) = if let Some(r) = line.strip_prefix("it(") {
        ("it ", r)
    } else if let Some(r) = line.strip_prefix("test(") {
        ("", r)
    } else {
        return None;
    };
    let q = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let body = &rest[1..];
    let end = body.find(q)?;
    let title = body[..end].trim();
    (!title.is_empty() && title.len() <= 120).then(|| format!("{kw}{title}"))
}

/// `(display name, 0-based line)` for each test case in a PHP test file.
pub fn parse(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut attr_test = false;
    let mut class = String::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("class ").or_else(|| line.strip_prefix("final class ")).or_else(|| line.strip_prefix("abstract class ")) {
            if let Some(n) = ident(rest) {
                class = n.to_string();
            }
            continue;
        }
        if line.starts_with("#[") && line.contains("Test") && !line.contains("TestWith") || line.contains("@test") {
            attr_test = true;
            continue;
        }
        if let Some(t) = pest_title(line) {
            out.push((t, i));
            attr_test = false;
            continue;
        }
        if let Some(pos) = line.find("function ") {
            let before = &line[..pos];
            if before.split_whitespace().all(|w| matches!(w, "public" | "static" | "final")) {
                if let Some(name) = ident(&line[pos + 9..]) {
                    if attr_test || name.starts_with("test") {
                        let shown = if class.is_empty() { name.to_string() } else { format!("{class}::{name}") };
                        out.push((shown, i));
                    }
                }
            }
            attr_test = false;
        } else if !line.starts_with("#[") && !line.starts_with('*') && !line.starts_with("/*") && !line.is_empty() {
            attr_test = false;
        }
    }
    out
}

pub fn discover(root: &Path) -> Vec<RouteTarget> {
    let mut files = Vec::new();
    walk(&root.join("tests"), 0, &mut files);
    let mut out = Vec::new();
    for f in files {
        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let is_file = is_test_file(name);
        let Ok(md) = std::fs::symlink_metadata(&f) else { continue };
        if !md.is_file() || md.len() > MAX_FILE {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        let cases = parse(&text);
        if !is_file && cases.is_empty() {
            continue;
        }
        let shown = f.strip_prefix(root).unwrap_or(&f).display().to_string();
        let stem = name.trim_end_matches(".php");
        out.push(RouteTarget { label: format!("test-file  {shown}"), path: f.clone(), line: 0 });
        for (case, line) in cases {
            let label = if case.contains("::") { format!("test  {case}") } else { format!("test  {stem}: {case}") };
            out.push(RouteTarget { label, path: f.clone(), line });
        }
        if out.len() >= MAX_ENTRIES {
            break;
        }
    }
    out.truncate(MAX_ENTRIES);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHPUNIT: &str = "<?php\nclass UserTest extends TestCase\n{\n    public function test_can_login(): void {}\n\n    /** @test */\n    public function it_logs_out() {}\n\n    #[Test]\n    public function registers(): void {}\n\n    public function helper() {}\n\n    #[DataProvider('x')]\n    public function test_with_data() {}\n}\n";

    #[test]
    fn phpunit_cases() {
        let got: Vec<_> = parse(PHPUNIT).into_iter().collect();
        assert_eq!(
            got,
            vec![
                ("UserTest::test_can_login".to_string(), 3),
                ("UserTest::it_logs_out".to_string(), 6),
                ("UserTest::registers".to_string(), 9),
                ("UserTest::test_with_data".to_string(), 14),
            ]
        );
    }

    #[test]
    fn pest_cases() {
        let t = "<?php\nuse App\\X;\n\nit('can login', function () {\n});\n\ntest(\"logs out\", fn () => 1);\n  it('')\n$x->it('nested')\n";
        assert_eq!(parse(t), vec![("it can login".to_string(), 3), ("logs out".to_string(), 6)]);
    }

    #[test]
    fn discover_end_to_end() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        std::fs::create_dir_all(r.join("tests/Feature")).unwrap();
        std::fs::create_dir_all(r.join("tests/vendor")).unwrap();
        std::fs::write(r.join("tests/Feature/UserTest.php"), PHPUNIT).unwrap();
        std::fs::write(r.join("tests/Feature/pest.php"), "<?php\nit('works', fn () => 1);\n").unwrap();
        std::fs::write(r.join("tests/Feature/Helper.php"), "<?php\nclass Helper {}\n").unwrap();
        std::fs::write(r.join("tests/vendor/XTest.php"), PHPUNIT).unwrap();
        let got = discover(r);
        let labels: Vec<_> = got.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels[0], "test-file  tests/Feature/UserTest.php");
        assert!(labels.contains(&"test  UserTest::registers"));
        assert!(labels.contains(&"test  pest: it works"));
        assert!(!labels.iter().any(|l| l.contains("Helper") || l.contains("XTest")));
        assert!(discover(&r.join("nope")).is_empty());
    }

    #[test]
    fn never_panics() {
        for t in ["é", "function ", "it('", "test(\"é", "#[", "@test", "class", "public function é() {}", "#[Test]\n"] {
            let _ = parse(t);
        }
        let big = "public function test_a() {}\n".repeat(200_000);
        let start = std::time::Instant::now();
        let _ = parse(&big);
        assert!(start.elapsed().as_secs() < 5);
    }
}
