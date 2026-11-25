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

/// `name` without `suffix` when it ends with it, matched
/// ASCII-case-insensitively. The stem keeps its own spelling; an empty stem
/// returns `None`.
pub(crate) fn strip_suffix_ignore_case<'a>(name: &'a str, suffix: &str) -> Option<&'a str> {
    let stem = name.get(..name.len().checked_sub(suffix.len())?)?;
    (!stem.is_empty() && name[name.len() - suffix.len()..].eq_ignore_ascii_case(suffix))
        .then_some(stem)
}

/// A Blade view file name without its `.blade.php` suffix, matched
/// case-insensitively: `input.BLADE.PHP` -> `input`. Backup suffixes and
/// plain `.php` files return `None`.
pub fn blade_view_stem(name: &str) -> Option<&str> {
    strip_suffix_ignore_case(name, ".blade.php")
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
    fn request_inspection_and_eloquent_group_keep_distinct_semantics() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        assert!(text("lv-request-is-method").contains("$request->isMethod('post')"));
        assert_eq!(text("lv-request-boolean"), "$archived = $request->boolean('archived');");
        let group = text("lv-eloquent-builder-group");
        assert!(group.contains("\\App\\Models\\Post::query()"));
        assert!(group.contains("\\Illuminate\\Database\\Eloquent\\Builder $query"));
        assert!(group.contains("$query->where('status', 'published')"));
        assert!(group.ends_with("->get();"));
    }

    #[test]
    fn destructive_migration_and_serialization_templates_expose_safety_limits() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        let drop_table = text("lv-migration-drop");
        assert!(drop_table.contains("Schema::dropIfExists('table')"));
        assert!(drop_table.contains("throw new \\LogicException("));
        assert!(!drop_table.contains("Schema::create("));
        assert_eq!(text("lv-model-to-array"), "$attributes = (clone $model)->setVisible(['id'])->toArray();");
        assert_eq!(text("lv-model-to-json"), "$json = (clone $model)->setVisible(['id'])->toJson(JSON_THROW_ON_ERROR);");
    }

    #[test]
    fn observer_registration_templates_preserve_qualified_names() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        assert_eq!(text("lv-observer-register"), "\\App\\Models\\Post::observe(\\App\\Observers\\PostObserver::class);");
        assert_eq!(text("lv-model-observed-by"), "#[\\Illuminate\\Database\\Eloquent\\Attributes\\ObservedBy([\\App\\Observers\\PostObserver::class])]");
    }

    #[test]
    fn html_resource_and_base_controller_templates_keep_distinct_roles() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        let resource = text("lv-controller-resource");
        for method in ["index", "create", "store", "show", "edit", "update", "destroy"] {
            assert!(resource.contains(&format!("function {method}(")));
        }
        assert_eq!(resource.matches("abort(501,").count(), 7);
        assert!(resource.contains("Post $post"));
        assert!(!resource.contains("$model"));
        assert!(resource.contains("\\Symfony\\Component\\HttpFoundation\\Response"));
        assert!(text("lv-controller-base").contains("abstract class Controller"));
        assert!(!text("lv-controller-base").contains("extends Controller"));
    }

    #[test]
    fn password_reset_template_uses_broker_validation_hash_and_token_rotation() {
        let item = packs().laravel.iter().find(|s| s.prefix.iter().any(|p| p == "lv-password-reset")).unwrap();
        let text = Parsed::from_str(&item.body).unwrap().text();
        assert!(text.contains("'confirmed'"));
        assert!(text.contains("Password::reset("));
        assert!(text.contains("Hash::make($password)"));
        assert!(text.contains("->setRememberToken(\\Illuminate\\Support\\Str::random(60))"));
        assert!(text.contains("PasswordReset($user)"));
    }

    #[test]
    fn bulk_update_and_model_safety_templates_have_explicit_guards() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        assert!(text("lv-query-update").contains("if ($ids === [])"));
        assert!(text("lv-query-update").contains("->whereIn('id', $ids)"));
        assert!(text("lv-model-guarded").contains("protected $guarded = ['*'];"));
        assert!(text("lv-model-boot").contains("parent::boot();"));
    }

    #[test]
    fn event_discovery_and_synchronous_broadcast_templates_keep_distinct_contracts() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        assert!(text("lv-event-discovery").contains("->withEvents(discover: ["));
        assert!(text("lv-event-discovery").contains("__DIR__.'/../app/Listeners'"));
        assert!(text("lv-event-broadcast-now").contains("implements ShouldBroadcastNow"));
        assert!(text("lv-event-broadcast-now").contains("new PrivateChannel('orders.'.$this->orderId)"));
    }

    #[test]
    fn conditional_validation_and_global_middleware_have_explicit_insertion_context() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        assert!(text("lv-validation-required-if").contains("\\Illuminate\\Validation\\Rule::requiredIf(false)"));
        assert!(text("lv-middleware-global").contains("$middleware->append(\\App\\Http\\Middleware\\InspectRequest::class)"));
    }

    #[test]
    fn existing_policy_and_cast_templates_have_safe_complete_defaults() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        let policy = text("lv-policy");
        assert_eq!(policy.matches("return false;").count(), 2);
        assert!(!policy.contains("user_id"));
        assert!(policy.contains("function view(User $user, Post $post): bool"));
        let cast = text("lv-cast");
        assert!(cast.contains("function get(\\Illuminate\\Database\\Eloquent\\Model $model"));
        assert!(cast.contains("function set(\\Illuminate\\Database\\Eloquent\\Model $model"));
    }

    #[test]
    fn email_verification_templates_keep_signed_auth_and_throttle_guards() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        let handler = text("lv-verification-handler");
        assert!(handler.contains("/email/verify/{id}/{hash}"));
        assert!(handler.contains("$request->fulfill()"));
        assert!(handler.contains("middleware(['auth', 'signed'])->name('verification.verify')"));
        assert!(text("lv-verification-resend").contains("middleware(['auth', 'throttle:6,1'])"));
        assert!(text("lv-verification-notice").contains("name('verification.notice')"));
        assert!(text("lv-route-verified").contains("middleware(['auth', 'verified'])"));
    }

    #[test]
    fn notification_channel_methods_keep_current_namespaces_and_signatures() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        assert!(text("lv-notification-mail").contains("toMail(object $notifiable): \\Illuminate\\Notifications\\Messages\\MailMessage"));
        assert!(text("lv-notification-slack").contains("toSlack(object $notifiable): \\Illuminate\\Notifications\\Slack\\SlackMessage"));
        assert!(text("lv-notification-slack").contains("->headerBlock('Notification')"));
        assert!(text("lv-notification-sms").contains("toVonage(object $notifiable): \\Illuminate\\Notifications\\Messages\\VonageMessage"));
    }

    #[test]
    fn controller_class_templates_fail_closed_until_application_logic_is_supplied() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        let invoke = text("lv-controller-invokable");
        assert!(invoke.contains("function __invoke(\\Illuminate\\Http\\Request $request): \\Illuminate\\Http\\JsonResponse"));
        assert!(invoke.contains("abort(501,"));
        let api = text("lv-controller-api-resource");
        for method in ["index", "store", "show", "update", "destroy"] { assert!(api.contains(&format!("function {method}("))); }
        assert!(!api.contains("function create("));
        assert!(api.contains("\\App\\Models\\Post $post"));
        assert_eq!(api.matches("abort(501,").count(), 5);
    }

    #[test]
    fn model_lifecycle_snippets_preserve_callback_scope_and_column_qualification() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        assert!(text("lv-model-booted").contains("protected static function booted(): void"));
        assert!(text("lv-model-creating").contains("static::creating(function (self $model): void"));
        assert!(text("lv-model-global-scope").contains("$builder->where('posts.published', true)"));
        assert!(text("lv-model-scope-class").contains("namespace App\\Models\\Scopes;"));
        assert!(text("lv-model-scope-class").contains("$model->qualifyColumn('published')"));
    }

    #[test]
    fn advanced_model_snippets_keep_collection_and_pivot_types() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        assert!(text("lv-model-ulid").contains("\\Illuminate\\Database\\Eloquent\\Concerns\\HasUlids"));
        assert!(text("lv-model-pivot").contains("class RoleUser extends \\Illuminate\\Database\\Eloquent\\Relations\\Pivot"));
        assert!(text("lv-model-pivot").contains("protected $fillable = ['active'];"));
        assert!(text("lv-model-collection-class").contains("namespace App\\Collections;"));
        assert!(text("lv-model-collected-by").contains("CollectedBy(\\App\\Collections\\PostCollection::class)"));
    }

    #[test]
    fn routing_followup_preserves_closing_braces_and_qualified_api_calls() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        assert!(text("lv-route-name-group").contains("Route::name('admin.')->group"));
        assert!(text("lv-rate-limiter").contains("$request->user()?->id ?: $request->ip()"));
        assert!(text("lv-route-throttled").contains("middleware('throttle:api')"));
        assert!(text("lv-route-signed").contains("Route::get('/action/{id}'"));
        assert!(text("lv-route-signed").contains("middleware('signed')->name('action')"));
        assert!(text("lv-url-signed").contains("['id' => $model->getKey()]"));
    }

    #[test]
    fn auth_snippets_preserve_session_safety_and_user_scoped_token_revocation() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        assert!(text("lv-auth-attempt").contains("$request->session()->regenerate();"));
        assert!(text("lv-auth-attempt").contains("->onlyInput('email')"));
        assert!(text("lv-auth-logout").contains("$request->session()->invalidate();"));
        assert!(text("lv-auth-logout").contains("$request->session()->regenerateToken();"));
        assert!(text("lv-auth-guard").contains("Auth::guard('web')->user()"));
        assert!(text("lv-password-send-reset-link").contains("Password::sendResetLink($request->only('email'))"));
        assert!(text("lv-sanctum-create-token").contains("['posts:read'], now()->addHours(1)"));
        assert!(text("lv-sanctum-revoke-token").contains("$request->user()->tokens()->where('id', $validated['token_id'])->delete()"));
    }

    #[test]
    fn mailable_snippets_preserve_types_variables_and_after_commit_dispatch() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        assert!(text("lv-mail-envelope").contains("function envelope(): \\Illuminate\\Mail\\Mailables\\Envelope"));
        assert!(text("lv-mail-content").contains("with: ['name' => $this->name]"));
        assert!(text("lv-mail-markdown").contains("markdown: 'mail.message'"));
        assert!(text("lv-mail-attachments").contains("Attachment::fromStorageDisk('local', 'reports/report.pdf')"));
        assert!(text("lv-mail-queue").contains("Mail::to($recipient)->queue("));
        assert!(text("lv-mail-queue").contains("OrderShipped($order))->afterCommit()"));
        assert!(text("lv-mail-later").contains("now()->addMinutes(10)"));
    }

    #[test]
    fn model_and_factory_snippets_keep_variables_types_and_namespace_separators() {
        let text = |prefix: &str| {
            let item = packs()
                .laravel
                .iter()
                .find(|s| s.prefix.iter().any(|p| p == prefix))
                .unwrap();
            Parsed::from_str(&item.body).unwrap().text()
        };
        assert!(text("lv-model-casts").contains("protected function casts(): array"));
        assert!(text("lv-model-enum-cast").contains("\\App\\Enums\\Status::class"));
        assert!(text("lv-model-soft-deletes").contains("\\Illuminate\\Database\\Eloquent\\SoftDeletes;"));
        assert!(text("lv-model-uuid").contains("\\Illuminate\\Database\\Eloquent\\Concerns\\HasUuids;"));
        assert!(text("lv-model-with").contains("protected $with = ['author'];"));
        assert!(text("lv-model-hidden").contains("protected $hidden = ['password', 'remember_token'];"));
        assert!(text("lv-model-attribute").contains("get: fn (string $value): string => ucfirst($value)"));
        assert!(text("lv-model-scope").contains("Builder $query): void"));
        assert!(text("lv-factory-state").contains("$this->state(fn (array $attributes): array"));
        assert!(text("lv-factory-configure").contains("function (\\App\\Models\\Post $model): void"));
    }

    #[test]
    fn laravel_family_prefixes_are_standardized_and_globally_unique() {
        let mut seen = std::collections::HashSet::new();
        for snippet in packs().laravel.iter().chain(&packs().livewire).chain(&packs().blade) {
            for prefix in &snippet.prefix {
                assert!(prefix.starts_with("lv-"), "{prefix}");
                assert!(seen.insert(prefix), "duplicate {prefix}");
            }
        }
    }

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
    fn blade_view_stem_accepts_suffix_case_variants() {
        assert_eq!(blade_view_stem("input.blade.php"), Some("input"));
        assert_eq!(blade_view_stem("forms/INPUT.BLADE.PHP"), Some("forms/INPUT"));
        assert_eq!(blade_view_stem("view.Blade.Php"), Some("view"));
        assert_eq!(blade_view_stem(".blade.php"), None);
        assert_eq!(blade_view_stem("input.blade.php.bak"), None);
        assert_eq!(blade_view_stem("input.php"), None);
        assert_eq!(strip_suffix_ignore_case("page.PHP", ".php"), Some("page"));
        assert_eq!(strip_suffix_ignore_case(".php", ".php"), None);
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
        assert!(got.iter().any(|s| s.prefix.iter().any(|x| x == "lv-route-get")));
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
        for want in ["lv-blade-foreach", "lv-blade-csrf", "lv-blade-wire-click", "lv-blade-livewire", "lv-blade-forelse"] {
            assert!(b.iter().any(|s| s.prefix.iter().any(|x| x == want)), "{want}");
        }
        // Blade directives are not offered in plain PHP or Twig files.
        assert!(!for_context("php", &none).iter().any(|s| s.prefix.iter().any(|x| x == "foreach")));
        let laravel = ProjectProfile { laravel: true, ..Default::default() };
        assert!(for_context("php", &laravel).iter().any(|s| s.prefix.iter().any(|x| x == "lv-livewire-component")));
        assert!(!for_context("php", &none).iter().any(|s| s.prefix.iter().any(|x| x == "lv-livewire-component")));
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
    fn case_variants_share_blade_completion_packs_and_component_context() {
        use std::path::Path;
        for name in ["view.blade.php", "VIEW.BLADE.PHP", "view.Blade.Php"] {
            let lang = snippet_language(Path::new(name), "php");
            assert_eq!(lang, "blade", "{name}");
            let pack = for_context(lang, &ProjectProfile::default());
            assert!(pack.iter().any(|s| s.prefix.iter().any(|p| p == "lv-blade-wire-target")));
            for text in ["<x-", "<x-forms.in", "<livewire:admin.us"] {
                assert!(lang == "blade" && tag_name_context(text), "{name}: {text}");
            }
        }
        assert_eq!(snippet_language(Path::new("BLADE.PHP.txt"), "php"), "php");
    }

    #[test]
    fn blade_bodies_keep_their_dollar_variables() {
        let each = packs().blade.iter().find(|s| s.prefix.iter().any(|x| x == "lv-blade-foreach")).unwrap();
        let text = Parsed::from_str(&each.body).unwrap().text();
        assert!(text.starts_with("@foreach ($items as $item)"), "{text}");
    }

    #[test]
    fn laravel_typed_relations_and_binding_keep_types_and_variables() {
        let text = |prefix: &str| {
            let s = packs().laravel.iter().find(|s| s.prefix.iter().any(|p| p == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        assert!(text("lv-hasmany").contains(": \\Illuminate\\Database\\Eloquent\\Relations\\HasMany"));
        assert!(text("lv-relation-morphto").contains("return $this->morphTo();"));
        assert!(text("lv-route-binding").contains("\\App\\Models\\Post $post"));
        assert!(text("lv-route-domain").contains("'{account}.example.com'"));
        assert!(text("lv-relation-latest-of-many").contains("->latestOfMany();"));
    }

    #[test]
    fn laravel_http_validation_templates_preserve_php_variables() {
        let text = |prefix: &str| {
            let s = packs().laravel.iter().find(|s| s.prefix.iter().any(|p| p == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        assert!(text("lv-response-json").contains("'data' => $data"));
        assert!(text("lv-http-get").contains("$response->json()"));
        assert!(text("lv-form-request-class").contains("\\App\\Models\\Post::class"));
        assert!(text("lv-validation-rule-class").contains("Closure $fail"));
        assert!(text("lv-form-request-after").contains("$validator->errors()->add"));
    }

    #[test]
    fn laravel_database_and_middleware_templates_keep_parameters() {
        let text = |prefix: &str| {
            let s = packs().laravel.iter().find(|s| s.prefix.iter().any(|p| p == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        assert!(text("lv-db-select").contains("[$id]"));
        assert!(text("lv-query-chunk-by-id").contains("\\App\\Models\\User::query()"));
        assert!(text("lv-middleware-parameter").contains("string $role"));
        assert!(text("lv-middleware-terminable").contains("terminate(Request $request, Response $response): void"));
        assert!(text("lv-migration-modify").contains("$table->dropColumn('nickname')"));
    }

    #[test]
    fn laravel_queue_auth_event_templates_preserve_php() {
        let text = |prefix: &str| {
            let s = packs().laravel.iter().find(|s| s.prefix.iter().any(|p| p == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        assert!(text("lv-job-unique").contains("return $this->recordId;"));
        assert!(text("lv-listener-queued").contains("\\App\\Events\\OrderPlaced $event"));
        assert!(text("lv-event-dispatch").contains("::dispatch($order)"));
        assert!(text("lv-gate-authorize").contains("'update', $post"));
        assert!(text("lv-notification-queued").contains("extends Notification implements ShouldQueue"));
    }

    #[test]
    fn laravel_pack_prefixes_are_unique_and_bodies_keep_php() {
        let mut seen = std::collections::HashSet::new();
        for s in &packs().laravel {
            for p in &s.prefix {
                assert!(seen.insert(p.clone()), "duplicate prefix {p}");
            }
        }
        for want in ["lv-job", "lv-event", "lv-listener", "lv-middleware", "lv-resource", "lv-policy", "lv-command", "lv-scope", "lv-pest", "lv-route-api", "lv-route-post", "lv-route-put", "lv-route-patch", "lv-route-delete", "lv-route-view", "lv-seeder", "lv-observer", "lv-notification", "lv-mail", "lv-cast", "lv-morphmany", "lv-schedule", "lv-cache"] {
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
        assert!(text_of("lv-route-post").contains("Route::post("));
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
        for want in ["lv-livewire-render", "lv-livewire-form", "lv-livewire-url", "lv-livewire-locked", "lv-livewire-modelable", "lv-livewire-session", "lv-livewire-test", "lv-blade-wire-navigate", "lv-blade-wire-confirm", "lv-blade-wire-dirty", "lv-blade-wire-offline", "lv-blade-wire-cloak"] {
            assert!(seen.contains(want), "{want}");
        }
        let text_of = |pack: &[Snippet], prefix: &str| {
            let s = pack.iter().find(|s| s.prefix.iter().any(|x| x == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        let form = text_of(&packs().livewire, "lv-livewire-form");
        assert!(form.contains("namespace App\\Livewire\\Forms;"), "{form}");
        assert!(form.contains("#[Validate('required|min:5')]"), "{form}");
        assert!(text_of(&packs().livewire, "lv-livewire-test").contains("->assertSee('text');"));
        assert!(text_of(&packs().livewire, "lv-livewire-url").contains("#[Url]"));
        assert!(text_of(&packs().blade, "lv-blade-wire-navigate").contains("wire:navigate href="));
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
        assert!(text_of("lv-blade-dd").contains("@dd("));
        assert!(text_of("lv-blade-dump").contains("@dump("));
        assert!(text_of("lv-blade-canany").contains("@endcanany"));
        assert!(text_of("lv-blade-cannot").contains("@endcannot"));
        assert!(text_of("lv-blade-each").contains("@each('view.name'"));
        assert!(text_of("lv-blade-includefirst").contains("@includeFirst("));
        assert_eq!(
            text_of("lv-blade-inject"),
            "@inject('name', 'App\\Services\\Service')"
        );
        assert_eq!(text_of("lv-blade-use"), "@use('App\\Models\\Model')");
        assert!(text_of("lv-blade-pushif").contains("@endPushIf"));
        assert!(text_of("lv-blade-prependonce").contains("@endPrependOnce"));
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
            "lv-blade-x-data", "lv-blade-x-init", "lv-blade-x-show", "lv-blade-x-on", "lv-blade-x-bind", "lv-blade-x-model", "lv-blade-x-text", "lv-blade-x-html",
            "lv-blade-x-ref", "lv-blade-x-cloak", "lv-blade-x-transition", "lv-blade-x-for", "lv-blade-x-if", "lv-blade-x-effect", "lv-blade-x-teleport",
            "lv-blade-x-ignore", "lv-blade-x-id", "lv-blade-x-modelable", "lv-blade-wire-init", "lv-blade-wire-ignore", "lv-blade-wire-stream",
            "lv-blade-wire-target", "lv-blade-wire-transition",
        ] {
            assert!(blade.iter().any(|s| s.prefix.iter().any(|p| p == want)), "{want}");
        }
        let body = |prefix: &str| {
            let s = blade.iter().find(|s| s.prefix.iter().any(|x| x == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        // x-on is used instead of the Blade-clashing `@click` shorthand.
        assert_eq!(body("lv-blade-x-on"), "x-on:click=\"handler\"");
        assert!(body("lv-blade-x-for").contains("x-for=\"item in items\""));
        assert!(body("lv-blade-x-data").contains("{ open: false }"));
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
        for want in ["lv-blade-pushonce", "lv-blade-prepend", "lv-blade-session", "lv-blade-checked", "lv-blade-selected", "lv-blade-disabled", "lv-blade-style", "lv-blade-aware", "lv-blade-fragment", "lv-blade-hassection", "lv-blade-links"] {
            assert!(seen.contains(want), "{want}");
        }
        let text_of = |prefix: &str| {
            let s = packs().blade.iter().find(|s| s.prefix.iter().any(|x| x == prefix)).unwrap();
            Parsed::from_str(&s.body).unwrap().text()
        };
        assert!(text_of("lv-blade-checked").starts_with("@checked($condition)"));
        assert!(text_of("lv-blade-session").contains("{{ $value }}"));
        assert!(text_of("lv-blade-links").contains("$items->links()"));
    }
}
