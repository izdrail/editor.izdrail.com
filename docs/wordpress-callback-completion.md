# WordPress callback completion

Typing the callback argument of a WordPress registration call offers
the plain functions the project itself defines (item 421).

## Where it works

- `add_action('hook', '...')` / `add_filter('hook', '...')` and the
  `remove_` / `has_` variants - the second argument.
- `add_shortcode('tag', '...')` - the second argument.
- `register_activation_hook(__FILE__, '...')`,
  `register_deactivation_hook(...)` and `register_uninstall_hook(...)`
  - the second argument.

## What is offered

Names harvested from plain `function name()` declarations across the
project's `.php` files (skipping `.git`, `node_modules` and `vendor`;
at most 500 files, 1 MB each). Textual scan; nothing is executed.
Class methods (`public function ...`), anonymous closures and names
from commented-out code or inside strings are not collected - a
method needs an array callable, which this list does not offer.
Unsaved edits are not seen until the lists cache expires.

Go to Definition on hooks, shortcodes and callbacks is separate
(see go-to-wordpress-hook).
