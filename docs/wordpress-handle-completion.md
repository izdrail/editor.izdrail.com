# WordPress script and style handle completion

Typing a script or style handle offers the handles the project
itself registers or enqueues (item 423).

## Where it works

- Scripts: `wp_register_script('...')`, `wp_enqueue_script(...)`,
  `wp_deregister_script(...)`, `wp_dequeue_script(...)`,
  `wp_script_is(...)` and `wp_localize_script(...)`.
- Styles: `wp_register_style('...')`, `wp_enqueue_style(...)`,
  `wp_deregister_style(...)`, `wp_dequeue_style(...)` and
  `wp_style_is(...)`.

In every case the handle is the first argument.

## What is offered

Handles harvested from all of the calls above across the project's
`.php` files (skipping `.git`, `node_modules` and `vendor`; at most
500 files, 1 MB each). Textual scan; nothing is executed. Handles
from commented-out code, from lookalike functions and from inside
strings are not collected. WordPress core handles (`jquery`,
`wp-emoji`, ...) are not listed - only handles the project itself
registers or enqueues. Unsaved edits are not seen until the lists
cache expires.
