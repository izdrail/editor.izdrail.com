//! Bundled snippet packs for web development, in VS Code snippet format.
//!
//! Packs are chosen from the file's language and the detected project profile:
//! Laravel, WordPress and Symfony snippets are offered for PHP only when the
//! project shows that framework, Twig snippets for `.twig` files.
use std::sync::OnceLock;

use lapce_core::project_profile::ProjectProfile;
use lapce_vsix::snippets::{self, Snippet};

const LARAVEL: &str = include_str!("../../defaults/snippets/laravel.json");
const WORDPRESS: &str = include_str!("../../defaults/snippets/wordpress.json");
const SYMFONY: &str = include_str!("../../defaults/snippets/symfony.json");
const TWIG: &str = include_str!("../../defaults/snippets/twig.json");

fn parse(text: &str) -> Vec<Snippet> {
    serde_json::from_str(text)
        .map(|doc| snippets::parse(&doc))
        .unwrap_or_default()
}

struct Packs {
    laravel: Vec<Snippet>,
    wordpress: Vec<Snippet>,
    symfony: Vec<Snippet>,
    twig: Vec<Snippet>,
}

fn packs() -> &'static Packs {
    static PACKS: OnceLock<Packs> = OnceLock::new();
    PACKS.get_or_init(|| Packs {
        laravel: parse(LARAVEL),
        wordpress: parse(WORDPRESS),
        symfony: parse(SYMFONY),
        twig: parse(TWIG),
    })
}

/// Bundled snippets that apply to `language_id` in a project with `profile`.
pub fn for_context(language_id: &str, profile: &ProjectProfile) -> Vec<&'static Snippet> {
    let p = packs();
    let mut out: Vec<&Snippet> = Vec::new();
    match language_id {
        "php" => {
            if profile.laravel {
                out.extend(&p.laravel);
            }
            if profile.wordpress {
                out.extend(&p.wordpress);
            }
            if profile.symfony_app {
                out.extend(&p.symfony);
            }
        }
        "twig" => out.extend(&p.twig),
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    use crate::snippet::Snippet as Parsed;

    #[test]
    fn every_bundled_snippet_parses_and_has_a_prefix() {
        let p = packs();
        for pack in [&p.laravel, &p.wordpress, &p.symfony, &p.twig] {
            assert!(pack.len() >= 8);
            for s in pack {
                assert!(!s.prefix.is_empty(), "{}", s.name);
                Parsed::from_str(&s.body)
                    .unwrap_or_else(|e| panic!("{}: {e}", s.name));
            }
        }
    }

    #[test]
    fn packs_follow_the_project_profile() {
        let none = ProjectProfile::default();
        assert!(for_context("php", &none).is_empty());
        let laravel = ProjectProfile {
            laravel: true,
            ..Default::default()
        };
        let got = for_context("php", &laravel);
        assert!(got.iter().any(|s| s.prefix.iter().any(|x| x == "route-get")));
        assert!(!got.iter().any(|s| s.prefix.iter().any(|x| x == "wp-action")));
        assert!(for_context("twig", &none).iter().any(|s| s.prefix.iter().any(|x| x == "for")));
        assert!(for_context("rust", &laravel).is_empty());
    }

    #[test]
    fn php_namespace_backslashes_survive() {
        let model = packs()
            .laravel
            .iter()
            .find(|s| s.prefix.iter().any(|x| x == "lv-model"))
            .unwrap();
        let text = Parsed::from_str(&model.body).unwrap().text();
        assert!(text.contains("namespace App\\Models;"), "{text}");
        assert!(text.contains("$fillable"), "{text}");
    }
}
