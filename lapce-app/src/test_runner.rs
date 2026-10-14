//! "Run Test at Cursor" / "Run Test File" for PHPUnit and Pest.
//!
//! Picks the runner from the project (`artisan` => `php artisan test`, else
//! `vendor/bin/pest`, else `vendor/bin/phpunit`) and finds the test around the
//! cursor textually: a PHPUnit method named `test*` (or marked `#[Test]` /
//! `@test`), or a top-level Pest `it('...')` / `test('...')` / `arch('...')`. Between tests, or
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

/// `(is_describe, description)` when `l` opens a Pest `it`/`test`/`describe`.
fn pest_opener(l: &str) -> Option<(bool, String)> {
    pest_opener_at(l, None)
}

/// Like `pest_opener`, but a call that ends at `(` (Pest code formatted with
/// the description on its own line) reads the description from `next`.
fn pest_opener_at(l: &str, next: Option<&str>) -> Option<(bool, String)> {
    let joined;
    let l = if l.trim_end().ends_with('(') && next.is_some() {
        joined = format!("{}{}", l.trim_end(), next.unwrap_or("").trim_start());
        joined.as_str()
    } else {
        l
    };
    let t = l.trim_start();
    for (call, describe) in
        [("it", false), ("test", false), ("arch", false), ("describe", true)]
    {
        if let Some(rest) = t.strip_prefix(call).and_then(|r| r.trim_start().strip_prefix('(')) {
            let rest = rest.trim_start();
            let q = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
            let body = &rest[1..];
            let d = body[..body.find(q)?].trim();
            return (!d.is_empty()).then(|| (describe, d.to_string()));
        }
    }
    None
}

fn indent_of(l: &str) -> usize {
    l.len() - l.trim_start().len()
}

/// Parentheses opened minus closed on a line (quotes ignored; only used to
/// spot one-line tests like `it('x', fn () => 1);`).
fn paren_balance(l: &str) -> i32 {
    l.chars().fold(0, |n, c| match c {
        '(' => n + 1,
        ')' => n - 1,
        _ => n,
    })
}

/// Description of the Pest `it`/`test` (or `describe` block) enclosing `line`.
/// Closing lines (`});`) and one-line tests above the cursor cancel the
/// opener at their own indent, so the cursor between tests finds the
/// surrounding `describe`, or nothing at the top level.
fn pest_description(lines: &[&str], line: usize) -> Option<String> {
    let mut i = line.min(lines.len().saturating_sub(1));
    let mut closed: Vec<usize> = Vec::new();
    loop {
        let l = lines[i];
        let above = i < line;
        if let Some((_, d)) = pest_opener_at(l, lines.get(i + 1).copied()) {
            let k = indent_of(l);
            let multi = l.trim_end().ends_with('(');
            if above && !multi && paren_balance(l) <= 0 {
                // A complete one-line test: not enclosing.
            } else if above && closed.contains(&k) {
                let pos = closed.iter().position(|c| *c == k).unwrap();
                closed.remove(pos);
            } else {
                return Some(d);
            }
        } else if above && l.trim_start().starts_with('}') {
            closed.push(indent_of(l));
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

    #[test]
    fn pest_describe_blocks_and_nested_tests() {
        let t = "<?php
describe('Auth', function () {
    it('logs in', function () {
        expect(1)->toBe(1);
    });

    it('is quick', fn () => 1);

    test('logs out', function () {
        //
    });
});

it('top level', function () {
});
";
        let lines: Vec<&str> = t.lines().collect();
        let d = |n: usize| pest_description(&lines, n);
        assert_eq!(d(3).as_deref(), Some("logs in")); // inside a nested test
        assert_eq!(d(2).as_deref(), Some("logs in"));
        assert_eq!(d(4).as_deref(), Some("logs in")); // its closing line
        assert_eq!(d(5).as_deref(), Some("Auth")); // blank between nested tests
        assert_eq!(d(6).as_deref(), Some("is quick")); // one-line test
        assert_eq!(d(7).as_deref(), Some("Auth")); // after a one-line test
        assert_eq!(d(9).as_deref(), Some("logs out"));
        assert_eq!(d(1).as_deref(), Some("Auth")); // on the describe line
        assert_eq!(d(11).as_deref(), Some("Auth")); // describe closing line
        assert_eq!(d(12), None); // between top-level blocks
        assert_eq!(d(14).as_deref(), Some("top level"));
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

    #[test]
    fn pest_arch_and_multiline_descriptions() {
        let src = "<?php
arch('no debug calls')
    ->expect(['dd', 'dump'])
    ->not->toBeUsed();

it(
    'creates a user',
    function () {
        expect(1)->toBe(1);
    }
);

test('other', fn () => 1);
";
        let lines: Vec<&str> = src.lines().collect();
        assert_eq!(pest_description(&lines, 1).as_deref(), Some("no debug calls"));
        assert_eq!(pest_description(&lines, 7).as_deref(), Some("creates a user"));
        assert_eq!(pest_description(&lines, 5).as_deref(), Some("creates a user"));
        assert_eq!(pest_description(&lines, 12).as_deref(), Some("other"));
        // Between tests: nothing encloses the cursor.
        assert_eq!(pest_description(&lines, 4), None);
    }

    #[test]
    fn dangling_open_paren_at_eof_is_safe() {
        let lines = ["it(", "'x'", "it("];
        for l in 0..3 {
            let _ = pest_description(&lines, l);
        }
    }
}
