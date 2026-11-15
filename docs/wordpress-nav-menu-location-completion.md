# WordPress nav menu location completion

Typing a nav menu location slug offers the locations the project
itself registers or uses (item 435).

## Where it works

- `has_nav_menu('...')` - the location is the first argument.
- `register_nav_menu('...', $description)` - same.

## What is offered

Location slugs harvested across the project's `.php` files
(skipping `.git`, `node_modules` and `vendor`; at most 500 files,
1 MB each) from three sources: the assoc keys of
`register_nav_menus(['primary' => ...])`, the first arguments of
`register_nav_menu` / `has_nav_menu`, and the `theme_location =>
...` values of `wp_nav_menu` calls. Textual scan; nothing is
executed. Slugs from commented-out code, from lookalike functions
and from inside strings are not collected. Unsaved edits are not
seen until the lists cache expires.
