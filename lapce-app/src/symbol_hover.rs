//! Hover a framework API name: the signature, pack provenance and
//! documentation link from the generated offline packs
//! (`metadata/web/symbols.json`, item 442). WordPress and Symfony
//! symbols stay gated on the detected project profile; Twig symbols
//! answer in `.twig` files only. Static lookup; nothing is executed.

use std::path::Path;

/// `.php` / `.blade.php` / `.twig` files.
pub fn supports_path(path: &Path) -> bool {
    crate::project_completion::php_completion_path(path)
        || path.extension().is_some_and(|e| e == "twig")
}

/// The identifier covering byte `col` of `line`, counting the byte
/// just past its end as inside (hover between word and delimiter).
fn word_at(line: &str, col: usize) -> Option<&str> {
    let mut col = col.min(line.len());
    while !line.is_char_boundary(col) {
        col -= 1;
    }
    let bytes = line.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let pos = if col < bytes.len() && is_ident(bytes[col]) {
        col
    } else if col > 0 && is_ident(bytes[col - 1]) {
        col - 1
    } else {
        return None;
    };
    let mut start = pos;
    while start > 0 && is_ident(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = pos + 1;
    while end < bytes.len() && is_ident(bytes[end]) {
        end += 1;
    }
    Some(&line[start..end])
}

/// Every pack name, built once: a word no pack knows must not cost
/// a project-profile read or a full symbol scan on each hover.
fn known_name(word: &str) -> bool {
    static NAMES: std::sync::OnceLock<
        std::collections::HashSet<&'static str>,
    > = std::sync::OnceLock::new();
    NAMES
        .get_or_init(|| {
            lapce_core::web_metadata::symbols()
                .iter()
                .map(|s| s.name.as_str())
                .collect()
        })
        .contains(word)
}

/// Markdown hover for the pack symbol named at byte `col` of `line`.
pub fn help(path: &Path, line: &str, col: usize) -> Option<String> {
    if line.len() > 8192 {
        return None;
    }
    let word = word_at(line, col)?;
    if !known_name(word) {
        return None;
    }
    let twig = path.extension().is_some_and(|e| e == "twig");
    let found = if twig {
        lapce_core::web_metadata::symbols()
            .iter()
            .find(|s| s.framework == "twig" && s.name == word)
    } else {
        let profile = lapce_core::project_profile::detect(path);
        lapce_core::web_metadata::php_symbols(&profile)
            .find(|s| s.name == word)
    }?;
    let mut md = String::new();
    if let Some(sig) = &found.signature {
        let lang = if twig { "twig" } else { "php" };
        md.push_str(&format!("```{lang}\n{sig}\n```\n"));
    }
    md.push_str(&format!(
        "**{}** ({}) - {} {} baseline\n{}",
        found.name, found.kind, found.package, found.version,
        found.doc_url
    ));
    Some(md)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_at_finds_identifiers() {
        assert_eq!(word_at("add_action('init')", 0), Some("add_action"));
        assert_eq!(word_at("add_action('init')", 3), Some("add_action"));
        // Just past the last byte still counts as the word.
        assert_eq!(word_at("add_action('init')", 10), Some("add_action"));
        // The quote between two words is neither.
        assert_eq!(word_at("add_action('init')", 11), None);
        assert_eq!(word_at("add_action('init')", 12), Some("init"));
        assert_eq!(word_at("add_action('init')", 16), Some("init"));
        assert_eq!(word_at("add_action('init')", 17), None);
        assert_eq!(word_at("", 0), None);
        assert_eq!(word_at("a", 5), Some("a"));
        // Multibyte text: a cursor inside `é` snaps to its start,
        // which is not an identifier byte.
        assert_eq!(word_at("é upper", 1), None);
        assert_eq!(word_at("é upper", 4), Some("upper"));
    }

    #[test]
    fn php_builtin_hover() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.php");
        std::fs::write(&f, "<?php\n").unwrap();
        let md = help(&f, "echo strlen($x);", 6).unwrap();
        assert!(md.contains("function strlen(string $string): int"));
        assert!(md.contains("**strlen** (function)"));
        assert!(md.contains("https://www.php.net/manual/en/function.strlen.php"));
        // An unknown name offers nothing.
        assert!(help(&f, "echo strl3n($x);", 6).is_none());
    }

    #[test]
    fn wordpress_symbols_follow_the_profile() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.php");
        std::fs::write(&f, "<?php\n").unwrap();
        // No WordPress evidence: its symbols stay out.
        assert!(help(&f, "add_action(", 3).is_none());
        // A wp-content directory marks the project WordPress.
        std::fs::create_dir(d.path().join("wp-content")).unwrap();
        let md = help(&f, "add_action(", 3).unwrap();
        assert!(md.contains("function add_action($hook_name"));
        assert!(md.contains("**add_action** (function)"));
        assert!(md.contains("https://developer.wordpress.org/reference/"));
    }

    #[test]
    fn twig_symbols_answer_in_twig_files() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.twig");
        std::fs::write(&f, "").unwrap();
        let md = help(&f, "{{ x|upper }}", 8).unwrap();
        assert!(md.contains("**upper** (filter)"));
        assert!(md.contains("https://twig.symfony.com/doc/3.x/"));
        // Twig names stay out of PHP files.
        let p = d.path().join("a.php");
        std::fs::write(&p, "<?php\n").unwrap();
        assert!(help(&p, "upper(", 2).is_none());
    }
}
