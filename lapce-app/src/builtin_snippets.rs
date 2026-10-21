//! Bundled snippet packs for web development, in VS Code snippet format.
//!
//! Packs are chosen from the file's language and the detected project profile:
//! Laravel, WordPress and Symfony snippets are offered for PHP only when the
//! project shows that framework, Twig snippets for `.twig` files, Blade
//! directive/Livewire snippets for `.blade.php` files and a core
//! JavaScript pack for `js`/`jsx`/`ts`/`tsx` files.
use std::sync::OnceLock;

use lapce_core::project_profile::ProjectProfile;
use lapce_vsix::snippets::{self, Snippet};

const LARAVEL: &str = include_str!("../../defaults/snippets/laravel.json");
const WORDPRESS: &str = include_str!("../../defaults/snippets/wordpress.json");
const SYMFONY: &str = include_str!("../../defaults/snippets/symfony.json");
const TWIG: &str = include_str!("../../defaults/snippets/twig.json");
const BLADE: &str = include_str!("../../defaults/snippets/blade.json");
const LIVEWIRE: &str = include_str!("../../defaults/snippets/livewire.json");
const JAVASCRIPT: &str = include_str!("../../defaults/snippets/javascript.json");

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

/// Bytes to extend a word start backwards over hyphenated segments, so
/// `x-d` and `wire-t` are completed as one word. `before` is the line text
/// before the current word start; it must end in `-` for anything to extend.
pub fn hyphen_word_len(before: &str) -> usize {
    let mut rest = before;
    let mut total = 0;
    while let Some(r) = rest.strip_suffix('-') {
        let word: usize = r
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .map(char::len_utf8)
            .sum();
        if word == 0 {
            break;
        }
        total += 1 + word;
        rest = &r[..r.len() - word];
    }
    total
}

/// Byte length of the Blade/Livewire component tag name being typed at the
/// end of `before` (the line text before the completion word start), so
/// `<x-forms.in` and `<livewire:admin.us` are completed as one dotted name.
/// Returns 0 outside a tag-name context.
pub fn tag_name_word_len(before: &str) -> usize {
    for marker in ["<x-", "<livewire:"] {
        if let Some(at) = before.rfind(marker) {
            let name = &before[at + marker.len()..];
            if !name.is_empty()
                && name.bytes().all(|b| {
                    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_')
                })
            {
                return name.len();
            }
        }
    }
    0
}

/// Is the completion word start (at the end of `before`, the line text
/// before it) inside a Blade component tag name, either right after the
/// `<x-` / `<livewire:` marker or further into the dotted name? There the
/// marker is not part of the word; elsewhere (`<div x-d`) a hyphenated
/// prefix is one word.
pub fn tag_name_context(before: &str) -> bool {
    before.ends_with("<x-")
        || before.ends_with("<livewire:")
        || tag_name_word_len(before) > 0
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
    javascript: Vec<Snippet>,
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
        javascript: parse(JAVASCRIPT),
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
        "javascript" | "javascriptreact" | "typescript" | "typescriptreact" => {
            out.extend(&p.javascript)
        }
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
        for pack in [&p.laravel, &p.wordpress, &p.symfony, &p.twig, &p.blade, &p.livewire, &p.javascript] {
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
    fn javascript_pack_covers_js_ts_jsx_tsx() {
        let none = ProjectProfile::default();
        for lang in ["javascript", "javascriptreact", "typescript", "typescriptreact"] {
            let got = for_context(lang, &none);
            for want in ["fn", "fetch", "map", "eli", "qs", "tc"] {
                assert!(got.iter().any(|s| s.prefix.iter().any(|x| x == want)), "{lang}: {want}");
            }
        }
        // The JS pack stays out of PHP, Blade and Twig.
        assert!(for_context("php", &none).is_empty());
        assert!(!for_context("blade", &none).iter().any(|s| s.prefix.iter().any(|x| x == "fetch")));
        assert!(!for_context("twig", &none).iter().any(|s| s.prefix.iter().any(|x| x == "fetch")));
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
    fn blade_directive_entries_parse() {
        let text_of = |prefix: &str| {
            let s = packs()
                .blade
                .iter()
                .find(|s| s.prefix.iter().any(|x| x == prefix))
                .unwrap_or_else(|| panic!("missing snippet {prefix}"));
            Parsed::from_str(&s.body).unwrap().text()
        };
        assert!(text_of("dd").contains("@dd("));
        assert!(text_of("dump").contains("@dump("));
        assert!(text_of("canany").contains("@endcanany"));
        assert!(text_of("cannot").contains("@endcannot"));
        assert!(text_of("each").contains("@each('view.name'"));
        assert!(text_of("includefirst").contains("@includeFirst("));
        assert_eq!(
            text_of("inject"),
            "@inject('name', 'App\\Services\\Service')"
        );
        assert_eq!(text_of("use"), "@use('App\\Models\\Model')");
        assert!(text_of("pushif").contains("@endPushIf"));
        assert!(text_of("prependonce").contains("@endPrependOnce"));
    }

    #[test]
    fn tag_name_word_len_covers_dotted_tag_names() {
        assert_eq!(tag_name_word_len("<x-"), 0);
        assert_eq!(tag_name_word_len("<x-forms."), 6);
        assert_eq!(tag_name_word_len("<x-forms.in"), 8);
        assert_eq!(tag_name_word_len("<livewire:"), 0);
        assert_eq!(tag_name_word_len("<livewire:admin."), 6);
        assert_eq!(tag_name_word_len("<livewire:admin.us"), 8);
        assert_eq!(tag_name_word_len("<livewire:admin.user-"), 11);
        // Attribute context is not a tag name; neither is a dotted path.
        assert_eq!(tag_name_word_len("<div x-"), 0);
        assert_eq!(tag_name_word_len("document."), 0);
        assert_eq!(tag_name_word_len("route('admin."), 0);
        assert_eq!(tag_name_word_len(""), 0);
    }

    #[test]
    fn tag_name_context_distinguishes_tag_from_attribute() {
        assert!(tag_name_context("<x-"));
        assert!(tag_name_context("  <x-"));
        assert!(tag_name_context("<x-forms."));
        assert!(tag_name_context("<livewire:"));
        assert!(tag_name_context("<livewire:admin."));
        // Alpine attribute context: the `x-` follows whitespace, not `<`.
        assert!(!tag_name_context("<div x-"));
        assert!(!tag_name_context("<x-alert x-"));
        assert!(!tag_name_context("document."));
        assert!(!tag_name_context(""));
    }

    #[test]
    fn hyphen_word_len_extends_over_hyphenated_segments() {
        assert_eq!(hyphen_word_len("<div x-"), 2);
        assert_eq!(hyphen_word_len("wire-"), 5);
        assert_eq!(hyphen_word_len("a-b-"), 4);
        assert_eq!(hyphen_word_len("<div "), 0);
        assert_eq!(hyphen_word_len("x "), 0);
        assert_eq!(hyphen_word_len("--"), 0);
        assert_eq!(hyphen_word_len(""), 0);
        assert_eq!(hyphen_word_len("é-"), 3);
    }

    #[test]
    fn alpine_and_extra_livewire_attributes_are_offered_in_blade() {
        let blade = &packs().blade;
        for want in [
            "x-data", "x-init", "x-show", "x-on", "x-bind", "x-model", "x-text", "x-html",
            "x-ref", "x-cloak", "x-transition", "x-for", "x-if", "x-effect", "x-teleport",
            "x-ignore", "x-id", "x-modelable", "wire-init", "wire-ignore", "wire-stream",
            "wire-target", "wire-transition",
        ] {
            assert!(blade.iter().any(|s| s.prefix.iter().any(|p| p == want)), "{want}");
        }
        let body = |prefix: &str| {
            let s = blade.iter().find(|s| s.prefix.iter().any(|x| x == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        // x-on is used instead of the Blade-clashing `@click` shorthand.
        assert_eq!(body("x-on"), "x-on:click=\"handler\"");
        assert!(body("x-for").contains("x-for=\"item in items\""));
        assert!(body("x-data").contains("{ open: false }"));
    }

    #[test]
    fn symfony_pack_prefixes_are_unique_and_new_snippets_keep_php() {
        let mut seen = std::collections::HashSet::new();
        for s in &packs().symfony {
            for p in &s.prefix {
                assert!(seen.insert(p.clone()), "duplicate prefix {p}");
            }
        }
        for want in ["sf-voter", "sf-test", "sf-messenger", "sf-repository", "sf-twigextension", "sf-listener", "sf-constraint", "sf-constraint-validator", "sf-authenticator", "sf-normalizer", "sf-twig-component", "sf-live-component", "sf-schedule-task", "sf-mail", "sf-event-listener"] {
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
        assert!(text_of("sf-constraint").contains("extends Constraint"));
        assert!(text_of("sf-constraint-validator").contains("->buildViolation($constraint->message)"));
        assert!(text_of("sf-authenticator").contains("extends AbstractAuthenticator"));
        assert!(text_of("sf-normalizer").contains("getSupportedTypes(?string $format): array"));
        assert!(text_of("sf-live-component").contains("use DefaultActionTrait;"));
        assert!(text_of("sf-schedule-task").contains("#[AsCronTask('0 0 * * *')]"));
        assert!(text_of("sf-mail").contains("new TemplatedEmail()"));
        assert!(text_of("sf-event-listener").contains("AsEventListener(event: KernelEvents::REQUEST"));
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
        for want in ["wp-taxonomy", "wp-metabox", "wp-ajax", "wp-nav-menu", "wp-image-size", "wp-remote-get", "wp-widget", "wp-cron", "wp-transient", "wp-option", "wp-settings", "wp-block", "wp-customizer", "wp-prepare", "wp-admin-page", "wp-pagination", "wp-localize"] {
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
        assert!(text_of("wp-transient").contains("$value = get_transient('key');"));
        assert!(text_of("wp-settings").contains("register_setting('my_group', 'my_option');"));
        assert!(text_of("wp-prepare").contains("$wpdb->prepare("));
        assert!(text_of("wp-customizer").contains("WP_Customize_Manager $wp_customize"));
        assert!(text_of("wp-block").contains("register_block_type("));
        assert!(text_of("wp-admin-page").contains("add_menu_page("));
        assert!(text_of("wp-pagination").contains("paginate_links("));
        assert!(text_of("wp-localize").contains("wp_localize_script("));
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
