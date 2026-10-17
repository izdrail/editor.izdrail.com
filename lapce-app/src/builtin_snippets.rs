//! Bundled snippet packs for web development, in VS Code snippet format.
//!
//! Packs are chosen from the file's language and the detected project profile:
//! Laravel, WordPress and Symfony snippets are offered for PHP only when the
//! project shows that framework, Twig snippets for `.twig` files and Blade
//! directive/Livewire snippets for `.blade.php` files.
use std::sync::OnceLock;

use lapce_core::project_profile::ProjectProfile;
use lapce_vsix::snippets::{self, Snippet};

const LARAVEL: &str = include_str!("../../defaults/snippets/laravel.json");
const WORDPRESS: &str = include_str!("../../defaults/snippets/wordpress.json");
const SYMFONY: &str = include_str!("../../defaults/snippets/symfony.json");
const TWIG: &str = include_str!("../../defaults/snippets/twig.json");
const BLADE: &str = include_str!("../../defaults/snippets/blade.json");
const LIVEWIRE: &str = include_str!("../../defaults/snippets/livewire.json");

/// The language id used for bundled snippets: `.blade.php` files are PHP to
/// the language server but get their own pack.
pub fn snippet_language<'a>(path: &std::path::Path, language_id: &'a str) -> &'a str {
    let blade = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.to_ascii_lowercase().ends_with(".blade.php"));
    if blade { "blade" } else { language_id }
}

/// Characters typed in front of the word that a snippet body repeats itself
/// (`@foreach`, `<livewire:`). The completion range is widened to cover one
/// so choosing the snippet does not leave `@@foreach`.
pub fn replaces_leading_char(language_id: &str, before: Option<char>) -> bool {
    language_id == "blade" && matches!(before, Some('@') | Some('<'))
}

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
    blade: Vec<Snippet>,
    livewire: Vec<Snippet>,
}

fn packs() -> &'static Packs {
    static PACKS: OnceLock<Packs> = OnceLock::new();
    PACKS.get_or_init(|| Packs {
        laravel: parse(LARAVEL),
        wordpress: parse(WORDPRESS),
        symfony: parse(SYMFONY),
        twig: parse(TWIG),
        blade: parse(BLADE),
        livewire: parse(LIVEWIRE),
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
                out.extend(&p.livewire);
            }
            if profile.wordpress {
                out.extend(&p.wordpress);
            }
            if profile.symfony_app {
                out.extend(&p.symfony);
            }
        }
        "twig" => out.extend(&p.twig),
        "blade" => out.extend(&p.blade),
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
        for pack in [&p.laravel, &p.wordpress, &p.symfony, &p.twig, &p.blade, &p.livewire] {
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

    #[test]
    fn blade_files_get_the_blade_pack_and_php_gets_livewire() {
        let none = ProjectProfile::default();
        let b = for_context("blade", &none);
        for want in ["foreach", "csrf", "wire-click", "livewire", "forelse"] {
            assert!(b.iter().any(|s| s.prefix.iter().any(|x| x == want)), "{want}");
        }
        // Blade directives are not offered in plain PHP or Twig files.
        assert!(!for_context("php", &none).iter().any(|s| s.prefix.iter().any(|x| x == "foreach")));
        let laravel = ProjectProfile { laravel: true, ..Default::default() };
        assert!(for_context("php", &laravel).iter().any(|s| s.prefix.iter().any(|x| x == "lw-component")));
        assert!(!for_context("php", &none).iter().any(|s| s.prefix.iter().any(|x| x == "lw-component")));
        assert!(for_context("twig", &none).iter().any(|s| s.prefix.iter().any(|x| x == "macro")));
    }

    #[test]
    fn blade_path_detection_and_leading_chars() {
        use std::path::Path;
        assert_eq!(snippet_language(Path::new("/p/resources/views/a.blade.php"), "php"), "blade");
        assert_eq!(snippet_language(Path::new("/p/A.BLADE.PHP"), "php"), "blade");
        assert_eq!(snippet_language(Path::new("/p/app/User.php"), "php"), "php");
        assert!(replaces_leading_char("blade", Some('@')));
        assert!(replaces_leading_char("blade", Some('<')));
        assert!(!replaces_leading_char("blade", Some(' ')));
        assert!(!replaces_leading_char("php", Some('@')));
        assert!(!replaces_leading_char("blade", None));
    }

    #[test]
    fn blade_bodies_keep_their_dollar_variables() {
        let each = packs().blade.iter().find(|s| s.prefix.iter().any(|x| x == "foreach")).unwrap();
        let text = Parsed::from_str(&each.body).unwrap().text();
        assert!(text.starts_with("@foreach ($items as $item)"), "{text}");
    }

    #[test]
    fn laravel_pack_prefixes_are_unique_and_bodies_keep_php() {
        let mut seen = std::collections::HashSet::new();
        for s in &packs().laravel {
            for p in &s.prefix {
                assert!(seen.insert(p.clone()), "duplicate prefix {p}");
            }
        }
        for want in ["lv-job", "lv-event", "lv-listener", "lv-middleware", "lv-resource", "lv-policy", "lv-command", "lv-scope", "lv-pest", "route-api", "route-post", "route-put", "route-patch", "route-delete", "route-view", "lv-seeder", "lv-observer", "lv-notification", "lv-mail", "lv-cast", "lv-morphmany", "lv-schedule", "lv-cache"] {
            assert!(seen.contains(want), "{want}");
        }
        let text_of = |prefix: &str| {
            let s = packs().laravel.iter().find(|s| s.prefix.iter().any(|x| x == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        let job = text_of("lv-job");
        assert!(job.contains("namespace App\\Jobs;"), "{job}");
        assert!(job.contains("use Illuminate\\Bus\\Queueable;"), "{job}");
        assert!(text_of("lv-policy").contains("User $user"));
        assert!(text_of("lv-resource").contains("$this->id"));
        assert!(text_of("route-post").contains("Route::post("));
        assert!(text_of("lv-cast").contains("array $attributes"));
        assert!(text_of("lv-mail").contains("new Envelope("));
        assert!(text_of("lv-notification").contains("new MailMessage"));
    }

    #[test]
    fn livewire_and_blade_new_entries_parse_and_keep_php() {
        let mut seen = std::collections::HashSet::new();
        for s in packs().livewire.iter().chain(&packs().blade) {
            for p in &s.prefix {
                assert!(seen.insert(p.clone()), "duplicate prefix {p}");
            }
        }
        for want in ["lw-render", "lw-form", "lw-url", "lw-locked", "lw-modelable", "lw-session", "lw-test", "wire-navigate", "wire-confirm", "wire-dirty", "wire-offline", "wire-cloak"] {
            assert!(seen.contains(want), "{want}");
        }
        let text_of = |pack: &[Snippet], prefix: &str| {
            let s = pack.iter().find(|s| s.prefix.iter().any(|x| x == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        let form = text_of(&packs().livewire, "lw-form");
        assert!(form.contains("namespace App\\Livewire\\Forms;"), "{form}");
        assert!(form.contains("#[Validate('required|min:5')]"), "{form}");
        assert!(text_of(&packs().livewire, "lw-test").contains("->assertSee('text');"));
        assert!(text_of(&packs().livewire, "lw-url").contains("#[Url]"));
        assert!(text_of(&packs().blade, "wire-navigate").contains("wire:navigate href="));
    }

    #[test]
    fn symfony_pack_prefixes_are_unique_and_new_snippets_keep_php() {
        let mut seen = std::collections::HashSet::new();
        for s in &packs().symfony {
            for p in &s.prefix {
                assert!(seen.insert(p.clone()), "duplicate prefix {p}");
            }
        }
        for want in ["sf-voter", "sf-test", "sf-messenger", "sf-repository", "sf-twigextension", "sf-listener"] {
            assert!(seen.contains(want), "{want}");
        }
        let text_of = |prefix: &str| {
            let s = packs().symfony.iter().find(|s| s.prefix.iter().any(|x| x == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        let voter = text_of("sf-voter");
        assert!(voter.contains("namespace App\\Security;"), "{voter}");
        assert!(voter.contains("class PostVoter extends Voter"), "{voter}");
        assert!(text_of("sf-test").contains("$client->request('GET', '/');"));
        assert!(text_of("sf-messenger").contains("class DoThingHandler"));
        assert!(text_of("sf-repository").contains("parent::__construct($registry, Product::class);"));
        assert!(text_of("sf-twigextension").contains("new TwigFilter('my_filter', $this->myFilter(...))"));
        assert!(text_of("sf-listener").contains("public function onKernelRequest(RequestEvent $event): void"));
    }

    #[test]
    fn wordpress_pack_prefixes_are_unique_and_bodies_keep_php() {
        let mut seen = std::collections::HashSet::new();
        for s in &packs().wordpress {
            for p in &s.prefix {
                assert!(seen.insert(p.clone()), "duplicate prefix {p}");
            }
        }
        for want in ["wp-taxonomy", "wp-metabox", "wp-ajax", "wp-nav-menu", "wp-image-size", "wp-remote-get", "wp-widget", "wp-cron"] {
            assert!(seen.contains(want), "{want}");
        }
        let text_of = |prefix: &str| {
            let s = packs().wordpress.iter().find(|s| s.prefix.iter().any(|x| x == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        assert!(text_of("wp-metabox").contains("get_post_meta($post->ID, '_my_box', true)"));
        assert!(text_of("wp-remote-get").contains("$response = wp_remote_get("));
        assert!(text_of("wp-widget").contains("class My_Widget extends WP_Widget {"));
        assert!(text_of("wp-cron").contains("wp_schedule_event(time(), 'hourly', 'my_cron');"));
    }

    #[test]
    fn blade_pack_prefixes_are_unique_and_new_directives_keep_variables() {
        let mut seen = std::collections::HashSet::new();
        for s in &packs().blade {
            for p in &s.prefix {
                assert!(seen.insert(p.clone()), "duplicate prefix {p}");
            }
        }
        for want in ["pushonce", "prepend", "session", "checked", "selected", "disabled", "style", "aware", "fragment", "hassection", "links"] {
            assert!(seen.contains(want), "{want}");
        }
        let text_of = |prefix: &str| {
            let s = packs().blade.iter().find(|s| s.prefix.iter().any(|x| x == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        assert!(text_of("checked").starts_with("@checked($condition)"));
        assert!(text_of("session").contains("{{ $value }}"));
        assert!(text_of("links").contains("$items->links()"));
    }
}
