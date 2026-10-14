//! "Run Test at Cursor" / "Run Test File" for PHPUnit and Pest.
//!
//! Picks the runner from the project (`artisan` => `php artisan test`, else
//! `vendor/bin/pest`, else `vendor/bin/phpunit`) and finds the test around the
//! cursor textually: a PHPUnit method named `test*` (or marked `#[Test]` /
//! `@test`), or a top-level Pest `it('...')` / `test('...')`. Between tests, or
//! with no match, the whole file runs. The command is only built here; the
//! caller starts it in the terminal when the user asks.
use std::path::{Path, PathBuf};

const MAX_NAME: usize = 200;

#[derive(Debug, PartialEq, Eq)]
pub struct TestRun {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// A file counts as a test when it lives under `tests/` (or `Tests/`) or is named `*Test.php`.
fn is_test_file(root: &Path, file: &Path) -> bool {
    if file.extension().is_none_or(|e| e != "php") {
        return false;
    }
    let rel = file.strip_prefix(root).unwrap_or(file);
    rel.components().any(|c| {
        let c = c.as_os_str().to_string_lossy();
        c == "tests" || c == "Tests"
    }) || file
        .file_name()
        .is_some_and(|n| n.to_string_lossy().ends_with("Test.php"))
}

/// Name of the PHPUnit test method enclosing `line`, if the cursor is in one.
fn phpunit_method(lines: &[&str], line: usize) -> Option<String> {
    let mut i = line.min(lines.len().saturating_sub(1));
    loop {
        let l = lines[i];
        if let Some(pos) = l.find("function ") {
            let before_ok = l[..pos].chars().next_back().is_none_or(|c| !is_ident(c));
            let name: String = l[pos + 9..].trim_start_matches('&').trim_start().chars().take_while(|c| is_ident(*c)).collect();
            if before_ok && !name.is_empty() {
                let marked = (i.saturating_sub(8)..i).any(|j| {
                    let t = lines[j].trim();
                    t.starts_with("#[Test") || t.contains("@test")
                });
                return (name.starts_with("test") || marked).then_some(name);
            }
        }
        // Between members: a closing brace at class-member indent above the cursor.
        if i < line {
            let indent = l.len() - l.trim_start().len();
            if l.trim() == "}" && indent <= 4 && (indent > 0 || l.starts_with('}')) {
                return None;
            }
        }
        if i == 0 {
            return None;
        }
        i -= 1;
    }
}

/// Description of the top-level Pest `it`/`test` enclosing `line`.
fn pest_description(lines: &[&str], line: usize) -> Option<String> {
    let mut i = line.min(lines.len().saturating_sub(1));
    loop {
        let l = lines[i];
        for call in ["it", "test"] {
            if let Some(rest) = l.strip_prefix(call).and_then(|r| r.trim_start().strip_prefix('(')) {
                let rest = rest.trim_start();
                let q = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
                let body = &rest[1..];
                let end = body.find(q)?;
                let d = body[..end].trim();
                return (!d.is_empty()).then(|| d.to_string());
            }
        }
        // A top-level closing line above the cursor means we are between tests.
        if i < line && (l.starts_with("});") || l.starts_with('}') || l.starts_with("describe(")) {
            return None;
        }
        if i == 0 {
            return None;
        }
        i -= 1;
    }
}

/// Regex-safe filter: metacharacters become `.` so they match anything.
fn filter_text(s: &str) -> String {
    s.chars()
        .map(|c| if "()[]{}.*+?^$|\\/#".contains(c) { '.' } else { c })
        .collect()
}

fn rel_arg(root: &Path, file: &Path) -> String {
    file.strip_prefix(root).unwrap_or(file).to_string_lossy().replace('\\', "/")
}

fn runner(root: &Path) -> Option<(String, Vec<String>)> {
    if root.join("artisan").is_file() {
        return Some(("php".into(), vec!["artisan".into(), "test".into()]));
    }
    let bin = root.join("vendor").join("bin");
    for name in ["pest", "phpunit"] {
        let p: PathBuf = bin.join(name);
        if p.is_file() {
            return Some((p.to_string_lossy().into_owned(), vec![]));
        }
    }
    None
}

/// The command to run for `file` (whole file when `line` is `None`).
pub fn plan(root: &Path, file: &Path, text: &str, line: Option<usize>) -> Result<TestRun, String> {
    if !is_test_file(root, file) {
        return Err("this is not a PHP test file (expected tests/ or *Test.php)".into());
    }
    let (program, mut args) = runner(root)
        .ok_or("no test runner found (artisan, vendor/bin/pest or vendor/bin/phpunit)")?;
    let rel = rel_arg(root, file);
    if rel.contains("${") {
        return Err("file path contains a variable marker".into());
    }
    let lines: Vec<&str> = text.lines().collect();
    let filter = line.filter(|_| !lines.is_empty()).and_then(|l| {
        phpunit_method(&lines, l)
            .or_else(|| pest_description(&lines, l))
            .filter(|n| n.len() <= MAX_NAME && !n.contains("${"))
    });
    args.push(rel.clone());
    let short = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(rel);
    let name = match &filter {
        Some(f) => {
            args.push("--filter".into());
            args.push(filter_text(f));
            format!("test: {short} {f}")
        }
        None => format!("test: {short}"),
    };
    Ok(TestRun { name, program, args })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHPUNIT: &str = "<?php
class UserTest extends TestCase
{
    use RefreshDatabase;

    public function testCreate(): void
    {
        $this->assertTrue(true);
    }

    public function helper(): int
    {
        return 1;
    }

    /** @test */
    public function it_deletes(): void
    {
        $x = 1;
    }

    #[Test]
    public function lists(): void {}
}
";

    #[test]
    fn phpunit_methods_around_the_cursor() {
        let lines: Vec<&str> = PHPUNIT.lines().collect();
        assert_eq!(phpunit_method(&lines, 7).as_deref(), Some("testCreate"));
        assert_eq!(phpunit_method(&lines, 5).as_deref(), Some("testCreate"));
        // The closing brace line of the method still belongs to it.
        assert_eq!(phpunit_method(&lines, 8).as_deref(), Some("testCreate"));
        assert_eq!(phpunit_method(&lines, 12).as_deref(), None); // helper
        assert_eq!(phpunit_method(&lines, 17).as_deref(), Some("it_deletes")); // @test
        assert_eq!(phpunit_method(&lines, 22).as_deref(), Some("lists")); // #[Test]
        assert_eq!(phpunit_method(&lines, 9).as_deref(), None); // blank line between methods
        assert_eq!(phpunit_method(&lines, 3).as_deref(), None); // class level
    }

    #[test]
    fn pest_tests_around_the_cursor() {
        let t = "<?php\n\nit('creates a user (admin)', function () {\n    expect(1)->toBe(1);\n});\n\ntest(\"lists\", function () {\n    //\n});\n";
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(pest_description(&lines, 3).as_deref(), Some("creates a user (admin)"));
        assert_eq!(pest_description(&lines, 2).as_deref(), Some("creates a user (admin)"));
        assert_eq!(pest_description(&lines, 7).as_deref(), Some("lists"));
        assert_eq!(pest_description(&lines, 5), None); // between tests
        assert_eq!(filter_text("creates a user (admin)"), "creates a user .admin.");
    }

    fn project(files: &[&str]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in files {
            let p = d.path().join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
        d
    }

    #[test]
    fn runner_choice_and_args() {
        let d = project(&["artisan", "vendor/bin/phpunit"]);
        let f = d.path().join("tests/Feature/UserTest.php");
        let r = plan(d.path(), &f, PHPUNIT, Some(7)).unwrap();
        assert_eq!(r.program, "php");
        assert_eq!(r.args, ["artisan", "test", "tests/Feature/UserTest.php", "--filter", "testCreate"]);
        assert_eq!(r.name, "test: UserTest.php testCreate");
        // Whole file, and between tests.
        let w = plan(d.path(), &f, PHPUNIT, None).unwrap();
        assert_eq!(w.args, ["artisan", "test", "tests/Feature/UserTest.php"]);
        assert_eq!(plan(d.path(), &f, PHPUNIT, Some(9)).unwrap().args.len(), 3);

        let d = project(&["vendor/bin/pest", "vendor/bin/phpunit"]);
        let r = plan(d.path(), &d.path().join("tests/a.php"), "<?php\nit('works', fn() => 1);\n", Some(1)).unwrap();
        assert!(r.program.ends_with("vendor/bin/pest"));
        assert_eq!(r.args, ["tests/a.php", "--filter", "works"]);

        let d = project(&["vendor/bin/phpunit"]);
        let r = plan(d.path(), &d.path().join("src/FooTest.php"), PHPUNIT, None).unwrap();
        assert!(r.program.ends_with("vendor/bin/phpunit"));
    }

    #[test]
    fn refuses_non_tests_and_missing_runner() {
        let d = project(&["artisan"]);
        assert!(plan(d.path(), &d.path().join("app/User.php"), "x", None).is_err());
        assert!(plan(d.path(), &d.path().join("tests/a.js"), "x", None).is_err());
        let d = project(&[]);
        assert!(plan(d.path(), &d.path().join("tests/aTest.php"), "x", None).is_err());
    }
}
