# WordPress post type name completion

Typing a post type name offers the types the project itself
registers (item 425).

## Where it works

- `register_post_type('...', ...)` - the name being registered.
- `unregister_post_type('...')`, `post_type_exists('...')` and
  `get_post_type_object('...')` - the name is the first argument.

## What is offered

Names harvested from `register_post_type('name', ...)` and
`unregister_post_type('name')` calls across the project's `.php`
files (skipping `.git`, `node_modules` and `vendor`; at most 500
files, 1 MB each). Textual scan; nothing is executed. Names from
commented-out code, from lookalike functions and from inside strings
are not collected. WordPress core types (`post`, `page`,
`attachment`, ...) are not listed - only types the project itself
registers. Post type names inside query arrays (`new WP_Query(...)`,
`get_posts(...)`) are a separate context and not covered here.
Unsaved edits are not seen until the lists cache expires.
