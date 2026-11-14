# WordPress hook name completion

Typing a hook argument in a WordPress project offers the hook names the
project itself registers or fires (item 419).

## Where it works

The first argument of the hook API calls:

- `add_action('...')` / `add_filter('...')` and
  `remove_action(...)` / `remove_filter(...)`
- `do_action('...')` / `apply_filters('...')` and the
  `do_action_ref_array(...)` / `apply_filters_ref_array(...)` variants
- `has_action('...')` / `has_filter('...')`,
  `doing_action('...')` / `doing_filter('...')`,
  `did_action('...')` / `did_filter('...')`,
  `current_action('...')` / `current_filter('...')`

## What is offered

Hook names harvested from the project's own `.php` files (skipping
`.git`, `node_modules` and `vendor`; at most 500 files, 1 MB each):
the quoted first arguments of `add_action` / `add_filter` /
`remove_action` / `remove_filter` / `do_action` / `apply_filters` and
the two `_ref_array` variants. Textual scan; nothing is executed.
Names from commented-out code, from lookalike functions
(`my_add_action(...)`) and from inside strings are not collected.
Core hooks are not listed - only names the project itself registers
or fires. Unsaved edits are not seen until the lists cache expires.

Go to Definition on hook names and callbacks is separate
(see go-to-wordpress-hook).

## Cron hook positions (item 427)

The same project hook list also completes the hook argument of the
cron API: `wp_schedule_event($timestamp, $recurrence, '...')` (third
argument), `wp_schedule_single_event($timestamp, '...')` (second),
and `wp_next_scheduled('...')`, `wp_clear_scheduled_hook('...')`,
`wp_unschedule_hook('...')` and `wp_get_scheduled_event('...')`
(first). Cron hooks are registered with `add_action` as usual, so
they are already in the harvested list.
