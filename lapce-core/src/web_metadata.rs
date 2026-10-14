//! Generated offline packs. These are declarations, not inferred runtime types.
use crate::project_profile::ProjectProfile;
use std::sync::OnceLock;

#[derive(Debug)]
pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub framework: String,
    pub package: String,
    pub version: String,
    pub signature: Option<String>,
    pub qualified_name: Option<String>,
    pub doc_url: String,
}

pub fn symbols() -> &'static [Symbol] {
    static PACK: OnceLock<Vec<Symbol>> = OnceLock::new();
    PACK.get_or_init(|| {
        let values: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../../metadata/web/symbols.json"))
                .expect("checked generated pack");
        values
            .iter()
            .map(|v| Symbol {
                name: v["name"].as_str().unwrap().to_owned(),
                kind: v["kind"].as_str().unwrap().to_owned(),
                framework: v["framework"].as_str().unwrap().to_owned(),
                package: v["package"].as_str().unwrap().to_owned(),
                version: v["source_version"].as_str().unwrap().to_owned(),
                signature: v["signature"].as_str().map(str::to_owned),
                qualified_name: v["qualified_name"].as_str().map(str::to_owned),
                doc_url: v["doc_url"].as_str().unwrap().to_owned(),
            })
            .collect()
    })
}

/// Packs are exact-version fallbacks, not a promise of latest API compatibility.
/// Unknown WP core version permits an explicitly labelled baseline; mismatched
/// installed Composer package versions are excluded rather than guessed.
pub fn php_symbols(
    profile: &ProjectProfile,
) -> impl Iterator<Item = &'static Symbol> + '_ {
    symbols()
        .iter()
        .filter(move |s| match s.framework.as_str() {
            // PHP standard library (phpstorm-stubs): applies to every PHP file.
            "php" => true,
            "wordpress" => {
                profile.wordpress
                    && profile
                        .wordpress_version
                        .as_ref()
                        .is_none_or(|v| v == &s.version)
                    && profile
                        .packages
                        .get(&s.package)
                        .is_none_or(|v| v.trim_start_matches('v') == s.version)
            }
            "symfony" => profile
                .packages
                .get(&s.package)
                .is_some_and(|v| v.trim_start_matches('v') == s.version),
            _ => false,
        })
}

pub fn php_lists(profile: &ProjectProfile) -> Vec<String> {
    let base = if profile.laravel {
        crate::lexilla::LARAVEL_WORD_LISTS
    } else {
        crate::lexilla::PHP_WORD_LISTS
    };
    let mut lists: Vec<String> = base.iter().map(|s| s.to_string()).collect();
    for symbol in php_symbols(profile) {
        let index = if symbol.kind == "class" { 2 } else { 1 };
        lists[index].push(' ');
        lists[index].push_str(&symbol.name);
    }
    lists
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packs_are_separate_versioned_and_nonempty() {
        let wp = ProjectProfile {
            wordpress: true,
            ..Default::default()
        };
        assert!(
            php_symbols(&wp)
                .any(|s| s.name == "add_action" && s.signature.is_some())
        );
        assert!(!php_symbols(&wp).any(|s| s.framework == "twig"));
        let mut sf = ProjectProfile::default();
        sf.packages
            .insert("symfony/console".into(), "v7.3.0".into());
        assert!(php_symbols(&sf).any(|s| s.name == "Command"));
        sf.packages
            .insert("symfony/console".into(), "v6.4.0".into());
        assert_eq!(php_symbols(&sf).filter(|s| s.framework != "php").count(), 0);
        let plain = ProjectProfile::default();
        for name in ["strlen", "array_map", "json_encode", "mb_strlen", "DateTime", "Countable"] {
            assert!(php_symbols(&plain).any(|s| s.name == name && s.framework == "php"), "{name}");
        }
        assert!(php_lists(&plain)[1].contains(" mb_strlen"));
        assert!(symbols().iter().any(|s| s.framework == "twig"
            && s.kind == "filter"
            && s.name == "upper"));
    }
}
