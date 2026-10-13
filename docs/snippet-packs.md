# Bundled snippet packs

Snippets for web developers ship with the editor and show in the completion
popup (snippet source, above buffer words) when the project matches:

| Pack | Offered when | Examples (prefix) |
| --- | --- | --- |
| Laravel | PHP file in a Laravel project | `route-get`, `route-resource`, `lv-model`, `lv-controller`, `lv-migration`, `lv-validate`, `lv-hasmany`, `lv-test` |
| WordPress | PHP file in a WordPress plugin/theme/core | `wp-action`, `wp-filter`, `wp-plugin`, `wp-cpt`, `wp-query`, `wp-enqueue`, `wp-shortcode`, `wp-rest` |
| Symfony | PHP file in a Symfony app | `sf-controller`, `sf-route`, `sf-entity`, `sf-command`, `sf-formtype`, `sf-subscriber` |
| Twig | `.twig` files | `for`, `if`, `ifelse`, `extends`, `block`, `include`, `set`, `path`, `asset`, `trans` |

Project detection is the static profile in `lapce_core::project_profile`
(Composer files, WordPress headers); nothing is executed. The packs are VS Code
snippet JSON in `defaults/snippets/` and are compiled into the binary. Snippets
imported from `.vsix` files are offered alongside them. Not included yet: Blade
directives (no Blade language id) and user-editable bundled packs.
