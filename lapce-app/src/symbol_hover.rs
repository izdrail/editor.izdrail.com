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
fn word_bounds(line: &str, col: usize) -> Option<(usize, usize)> {
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
    Some((start, end))
}

#[cfg(test)]
fn word_at(line: &str, col: usize) -> Option<&str> {
    let (start, end) = word_bounds(line, col)?;
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
pub fn help_with_profile(
    path: &Path,
    line: &str,
    col: usize,
    profile: &lapce_core::project_profile::ProjectProfile,
) -> Option<String> {
    if line.len() > 8192 {
        return None;
    }
    let (start, end) = word_bounds(line, col)?;
    let word = &line[start..end];
    if !known_name(word) {
        return None;
    }
    let twig = path.extension().is_some_and(|e| e == "twig");
    let found = if twig {
        let kind = lapce_core::web_templates::completion_kind(&line[..end])?;
        lapce_core::web_metadata::symbols()
            .iter()
            .find(|s| s.framework == "twig" && s.name == word && s.kind == kind)
    } else {
        // This provider receives one PHP source line, usually without its
        // document's opening tag. Give the shared lexer that missing mode.
        let prefix = &line[..end];
        let php_source = if prefix.contains("<?") {
            std::borrow::Cow::Borrowed(prefix)
        } else {
            std::borrow::Cow::Owned(format!("<?php {prefix}"))
        };
        if !lapce_core::web_templates::php_completion_context(&php_source) {
            return None;
        }
        lapce_core::web_metadata::php_symbols(profile)
            .find(|s| s.name == word)
    }?;
    let mut md = String::new();
    if let Some(sig) = &found.signature {
        let lang = if twig { "twig" } else { "php" };
        md.push_str(&format!("```{lang}\n{sig}\n```\n"));
    }
    // A real markdown link: clickable where links open (item 443).
    md.push_str(&format!(
        "**{}** ({}) - {} {} baseline\n[{}]({})",
        found.name, found.kind, found.package, found.version,
        found.doc_url, found.doc_url
    ));
    Some(md)
}

// Standalone fuzz/tests may discover a profile, never interactive hover.
#[cfg(test)]
pub fn help(path: &Path, line: &str, col: usize) -> Option<String> {
    help_with_profile(path, line, col, &lapce_core::project_profile::detect(path))
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
    fn catalogue_hover_respects_code_context_and_twig_symbol_kind() {
        let php = Path::new("probe.php");
        for line in ["$strlen = 1;", "$x->strlen()", "Type::strlen()", "echo 'strlen';", "// strlen($x)"] {
            let col = line.find("strlen").unwrap() + 2;
            assert!(help(php, line, col).is_none(), "{line}");
        }
        let twig = Path::new("probe.twig");
        for line in ["<p>upper</p>", "{{ 'upper' }}", "{# upper #}", "{{ user.upper }}", "{{ upper }}"] {
            let col = line.find("upper").unwrap() + 2;
            assert!(help(twig, line, col).is_none(), "{line}");
        }
        for (line, word, kind) in [
            ("{{ value|upper }}", "upper", "filter"),
            ("{{ include('x') }}", "include", "function"),
            ("{% include 'x' %}", "include", "tag"),
            ("{{ user is defined }}", "defined", "test"),
        ] {
            let md = help(twig, line, line.find(word).unwrap() + 2).unwrap();
            assert!(md.contains(&format!("({kind})")), "{line}: {md}");
        }
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
