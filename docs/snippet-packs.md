# Bundled snippet packs

Snippets for web developers ship with the editor and show in the completion
popup (snippet source, above buffer words) when the project matches:

| Pack | Offered when | Examples (prefix) |
| --- | --- | --- |
| Laravel | PHP file in a Laravel project | `route-get`, `route-resource`, `lv-model`, `lv-controller`, `lv-migration`, `lv-validate`, `lv-hasmany`, `lv-test` |
| WordPress | PHP file in a WordPress plugin/theme/core | `wp-action`, `wp-filter`, `wp-plugin`, `wp-cpt`, `wp-query`, `wp-enqueue`, `wp-shortcode`, `wp-rest` |
| Symfony | PHP file in a Symfony app | `sf-controller`, `sf-route`, `sf-entity`, `sf-command`, `sf-formtype`, `sf-subscriber` |
| Twig | `.twig` files | `for`, `forelse`, `if`, `extends`, `block`, `include`, `includewith`, `macro`, `import`, `embed`, `set`, `path`, `pathp`, `asset`, `trans`, `form`, `form_row`, `csrf`, `flash`, `isgranted`, `twig-component` |
| Blade | `.blade.php` files | `if`, `foreach`, `forelse`, `auth`, `can`, `section`, `extends`, `include`, `props`, `csrf`, `method`, `error`, `class`, `vite`, `echo`, `raw`, `x-`, `x-slot`, `livewire`, `wire-click`, `wire-model`, `wire-model-live`, `wire-submit`, `wire-loading`, `wire-poll`, `entangle` |
| Livewire | PHP file in a Laravel project | `lw-component`, `lw-rule`, `lw-on`, `lw-computed`, `lw-dispatch`, `lw-mount`, `lw-redirect` |

Project detection is the static profile in `lapce_core::project_profile`
(Composer files, WordPress headers); nothing is executed. The packs are VS Code
snippet JSON in `defaults/snippets/` and are compiled into the binary. Snippets
imported from `.vsix` files are offered alongside them. In Blade files the snippet
replaces a typed `@` or `<` in front of the prefix (`@fo` + `foreach` gives `@foreach (...)`). Blade
files do not get the PHP/Laravel pack, since they are templates. Livewire snippets target Livewire 3
(`#[Rule]`, `#[On]`, `dispatch`); `wire-submit-prevent` is the v2 form. Not included yet: user-editable
bundled packs.
