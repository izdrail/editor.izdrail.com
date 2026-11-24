# Bundled snippet packs

Snippets for web developers ship with the editor and show in the completion
popup (snippet source, above buffer words) when the project matches:

| Pack | Offered when | Examples (prefix) |
| --- | --- | --- |
| Laravel | PHP file in a Laravel project | `lv-route-get`, `lv-model`, `lv-controller`, `lv-relation-morphto`; see [all prefixes](usage/snippet-reference.md#laravel) |
| WordPress | PHP file in a WordPress plugin/theme/core | `wp-action`, `wp-filter`, `wp-plugin`, `wp-cpt`, `wp-query`, `wp-enqueue`, `wp-shortcode`, `wp-rest`, `wp-esc`, `wp-nonce`, `wp-i18n`, `wp-taxonomy`, `wp-metabox`, `wp-ajax`, `wp-nav-menu`, `wp-image-size`, `wp-remote-get`, `wp-widget`, `wp-cron`, `wp-transient`, `wp-option`, `wp-settings`, `wp-block`, `wp-customizer`, `wp-prepare`, `wp-admin-page`, `wp-pagination`, `wp-localize` |
| Symfony | PHP file in a Symfony app | `sf-controller`, `sf-route`, `sf-entity`, `sf-column`, `sf-service`, `sf-command`, `sf-formtype`, `sf-subscriber`, `sf-voter`, `sf-test`, `sf-messenger`, `sf-repository`, `sf-twigextension`, `sf-listener`, `sf-constraint`, `sf-constraint-validator`, `sf-authenticator`, `sf-normalizer`, `sf-twig-component`, `sf-live-component`, `sf-schedule-task`, `sf-mail`, `sf-event-listener` |
| Twig | `.twig` files | `for`, `forelse`, `if`, `extends`, `block`, `include`, `includewith`, `macro`, `import`, `embed`, `set`, `path`, `pathp`, `asset`, `trans`, `form`, `form_row`, `csrf`, `flash`, `isgranted`, `twig-component` |
| Blade | `.blade.php` files | `lv-blade-if`, `lv-blade-foreach`; see [all prefixes](usage/snippet-reference.md#blade) |
| Livewire | PHP file in a Laravel project | `lv-livewire-component`, `lv-livewire-rule`; see [all prefixes](usage/snippet-reference.md#livewire) |
| JavaScript | `.js` / `.jsx` / `.ts` / `.tsx` files (any project) | `fn`, `afn`, `arrow`, `imp`, `impn`, `expd`, `fetch`, `map`, `filter`, `reduce`, `foreach`, `eli`, `qs`, `qsa`, `cl`, `prom`, `tc`, `class` |

Project detection is the static profile in `lapce_core::project_profile`
(Composer files, WordPress headers); nothing is executed. The packs are VS Code
snippet JSON in `defaults/snippets/` and are compiled into the binary. Snippets
imported from `.vsix` files are offered alongside them. In Blade files the snippet
replaces a typed `@` or `<` in front of the prefix (`@lv-blade-fo` + `lv-blade-foreach` gives `@foreach (...)`). Blade
files do not get the PHP/Laravel pack, since they are templates. Livewire snippets target Livewire 3
(`#[Rule]`, `#[On]`, `dispatch`); `wire-submit-prevent` is the v2 form. Not included yet: user-editable
bundled packs.
