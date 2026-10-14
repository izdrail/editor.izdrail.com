//! "Organize PHP Imports": removes unused `use` lines and sorts the rest.
//!
//! Works on runs of consecutive plain class imports (`use A\B;`, `use A\B as
//! C;`) above the first class declaration. Blank lines between runs are kept
//! as the user's grouping, so each run is cleaned and sorted on its own.
//! Group imports (`use A\{B, C};`), `use function` and `use const` lines are
//! left alone. A name counts as used when it appears as a word anywhere else
//! in the file (code, strings or docblocks), compared case-insensitively. This
//! errs towards keeping an import.
use std::{collections::HashSet, ops::Range};

const MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Organized {
    /// Byte ranges to replace, ascending and non-overlapping.
    pub edits: Vec<(Range<usize>, String)>,
    pub removed: Vec<String>,
    pub sorted_runs: usize,
}

struct Import {
    line: String,
    key: String,
    short: String,
}

fn parse_import(line: &str) -> Option<Import> {
    let t = line.trim();
    let body = t.strip_prefix("use ")?.strip_suffix(';')?.trim();
    if body.contains(['{', ',', '(']) || body.starts_with("function ") || body.starts_with("const ") {
        return None;
    }
    let (full, alias) = match body.split_once(" as ") {
        Some((f, a)) => (f.trim(), Some(a.trim())),
        None => (body, None),
    };
    if full.is_empty() || full.contains(char::is_whitespace) {
        return None;
    }
    let full = full.trim_start_matches('\\');
    let short = alias.unwrap_or_else(|| full.rsplit('\\').next().unwrap_or(full));
    if short.is_empty() || !short.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some(Import {
        line: t.to_string(),
        key: format!("{}\u{0}{}", full.to_lowercase(), alias.unwrap_or("").to_lowercase()),
        short: short.to_lowercase(),
    })
}

fn words(text: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        let w = c.is_alphanumeric() || c == '_';
        match (w, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.insert(text[s..i].to_lowercase());
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.insert(text[s..].to_lowercase());
    }
    out
}

pub fn organize(text: &str) -> Option<Organized> {
    if text.len() > MAX_BYTES {
        return None;
    }
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    // Lines with their byte spans, up to the first declaration.
    let mut lines: Vec<(Range<usize>, &str)> = vec![];
    let mut pos = 0;
    for raw in text.split_inclusive('\n') {
        let start = pos;
        pos += raw.len();
        let content = raw.trim_end_matches(['\r', '\n']);
        lines.push((start..start + content.len(), content));
    }
    let decl = ["class ", "abstract ", "final ", "interface ", "trait ", "enum ", "readonly ", "function "];
    let header_end = lines
        .iter()
        .position(|(_, l)| decl.iter().any(|d| l.trim_start().starts_with(d)))
        .unwrap_or(lines.len());
    // Text that can use an import: everything but the header's use/namespace lines.
    let mut body = String::new();
    for (i, (_, l)) in lines.iter().enumerate() {
        let t = l.trim_start();
        if i < header_end && (t.starts_with("use ") || t.starts_with("namespace ")) {
            continue;
        }
        body.push_str(l);
        body.push('\n');
    }
    let used = words(&body);

    let mut result = Organized::default();
    let mut i = 0;
    while i < header_end {
        if parse_import(lines[i].1).is_none() {
            i += 1;
            continue;
        }
        let mut j = i;
        let mut run = vec![];
        while j < header_end {
            match parse_import(lines[j].1) {
                Some(imp) => {
                    run.push(imp);
                    j += 1;
                }
                None => break,
            }
        }
        let original: Vec<String> = run.iter().map(|r| r.line.clone()).collect();
        let mut kept: Vec<Import> = vec![];
        let mut seen = HashSet::new();
        for imp in run {
            if !used.contains(&imp.short) {
                result.removed.push(imp.line);
            } else if seen.insert(imp.key.clone()) {
                kept.push(imp);
            }
        }
        kept.sort_by(|a, b| a.key.cmp(&b.key));
        let new: Vec<String> = kept.into_iter().map(|k| k.line).collect();
        if new != original {
            if new.len() == original.len() {
                result.sorted_runs += 1;
            }
            let start = lines[i].0.start;
            let last = &lines[j - 1].0;
            if new.is_empty() {
                // Drop the lines and the line break after them, plus one
                // following blank line so no double gap is left.
                let mut end = last.end;
                end += if text[end..].starts_with("\r\n") { 2 } else { usize::from(text[end..].starts_with('\n')) };
                if j < lines.len() && lines[j].1.trim().is_empty() && j + 1 < lines.len() {
                    end = lines[j].0.end;
                    end += if text[end..].starts_with("\r\n") { 2 } else { usize::from(text[end..].starts_with('\n')) };
                }
                result.edits.push((start..end, String::new()));
            } else {
                result.edits.push((start..last.end, new.join(nl)));
            }
        }
        i = j;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(text: &str) -> (String, Organized) {
        let o = organize(text).unwrap();
        let mut out = text.to_string();
        for (r, s) in o.edits.iter().rev() {
            out.replace_range(r.clone(), s);
        }
        (out, o)
    }

    const HEAD: &str = "<?php\n\nnamespace App\\Http;\n\n";

    #[test]
    fn sorts_case_insensitively() {
        let src = format!("{HEAD}use Illuminate\\Http\\Request;\nuse App\\Models\\User;\nuse app\\Models\\Post;\n\nclass A {{ function f(Request $r, User $u, Post $p) {{}} }}\n");
        let (out, o) = apply(&src);
        assert!(out.contains("use app\\Models\\Post;\nuse App\\Models\\User;\nuse Illuminate\\Http\\Request;\n"), "{out}");
        assert!(o.removed.is_empty());
        assert_eq!(o.sorted_runs, 1);
    }

    #[test]
    fn removes_unused_and_keeps_docblock_use() {
        let src = format!("{HEAD}use A\\Used;\nuse A\\Unused;\nuse A\\InDoc;\nuse A\\Aliased as Al;\nuse A\\UnusedAlias as Nope;\n\n/** @var InDoc */\nclass X {{ function f(Used $u, Al $a) {{}} }}\n");
        let (out, o) = apply(&src);
        assert_eq!(o.removed, vec!["use A\\Unused;", "use A\\UnusedAlias as Nope;"]);
        assert!(out.contains("use A\\Aliased as Al;\nuse A\\InDoc;\nuse A\\Used;\n"), "{out}");
        assert!(!out.contains("Unused"));
    }

    #[test]
    fn removes_duplicates() {
        let src = format!("{HEAD}use A\\B;\nuse A\\B;\n\nclass X {{ B $b; }}\n");
        let (out, _) = apply(&src);
        assert_eq!(out.matches("use A\\B;").count(), 1);
    }

    #[test]
    fn runs_separated_by_blank_lines_stay_separate() {
        let src = format!("{HEAD}use Z\\Z;\nuse A\\A;\n\nuse M\\M;\nuse B\\B;\n\nclass X {{ Z A M B }}\n");
        let (out, _) = apply(&src);
        assert!(out.contains("use A\\A;\nuse Z\\Z;\n\nuse B\\B;\nuse M\\M;\n"), "{out}");
    }

    #[test]
    fn all_unused_run_is_removed_without_double_gap() {
        let src = format!("{HEAD}use A\\B;\nuse C\\D;\n\nclass X {{}}\n");
        let (out, o) = apply(&src);
        assert_eq!(out, format!("{HEAD}class X {{}}\n"));
        assert_eq!(o.removed.len(), 2);
    }

    #[test]
    fn group_function_const_and_trait_use_are_untouched() {
        let src = format!("{HEAD}use A\\{{B, C}};\nuse function A\\f;\nuse const A\\K;\nuse Z\\Z;\nuse Y\\Y;\n\nclass X {{\n    use Y;\n    function f(Z $z) {{ f(); K; B; }}\n}}\n");
        let (out, _) = apply(&src);
        assert!(out.contains("use A\\{B, C};\nuse function A\\f;\nuse const A\\K;\nuse Y\\Y;\nuse Z\\Z;\n"), "{out}");
    }

    #[test]
    fn already_organized_has_no_edits() {
        let src = format!("{HEAD}use A\\A;\nuse B\\B;\n\nclass X {{ A B }}\n");
        assert_eq!(organize(&src).unwrap(), Organized::default());
    }

    #[test]
    fn crlf_is_kept() {
        let src = "<?php\r\nnamespace N;\r\nuse Z\\Z;\r\nuse A\\A;\r\nclass X { Z A }\r\n";
        let (out, _) = apply(src);
        assert_eq!(out, "<?php\r\nnamespace N;\r\nuse A\\A;\r\nuse Z\\Z;\r\nclass X { Z A }\r\n");
    }

    #[test]
    fn no_header_and_big_and_odd_inputs() {
        assert_eq!(organize("<?php\nclass A {}\n").unwrap(), Organized::default());
        assert_eq!(organize("").unwrap(), Organized::default());
        assert!(organize(&"a".repeat(2 * 1024 * 1024)).is_none());
        let _ = organize("use \nuse ;\nuse A as ;\nuse é\\ü;\nclass é {}");
        let many: String = (0..5000).map(|i| format!("use A\\C{i};\n")).collect();
        let (_, o) = apply(&format!("<?php\n{many}class X {{}}\n"));
        assert_eq!(o.removed.len(), 5000);
    }
}
