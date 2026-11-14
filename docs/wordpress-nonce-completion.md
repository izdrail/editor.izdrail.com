# WordPress nonce action name completion

Typing a nonce action name offers the actions the project itself
creates or checks (item 429).

## Where it works

- `wp_create_nonce('...')`, `check_admin_referer('...')`,
  `check_ajax_referer('...')` and `wp_nonce_field('...')` - the
  action is the first argument.
- `wp_verify_nonce($nonce, '...')` and `wp_nonce_url($url, '...')` -
  the action is the second argument.

## What is offered

Actions harvested from `wp_create_nonce`, `check_admin_referer`,
`check_ajax_referer` and `wp_nonce_field` calls across the project's
`.php` files (skipping `.git`, `node_modules` and `vendor`; at most
500 files, 1 MB each). Textual scan; nothing is executed. Actions
from commented-out code, from lookalike functions and from inside
strings are not collected. WordPress core actions (`-1` and friends)
are not listed - only actions the project itself creates or checks.
Unsaved edits are not seen until the lists cache expires.
