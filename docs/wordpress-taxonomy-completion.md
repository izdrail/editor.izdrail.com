# WordPress taxonomy name completion

Typing a taxonomy name offers the taxonomies the project itself
registers (item 426).

## Where it works

- `register_taxonomy('...', ...)` - the name being registered.
- `unregister_taxonomy('...')`, `taxonomy_exists('...')`,
  `get_taxonomy('...')` and
  `register_taxonomy_for_object_type('...', ...)` - the name is the
  first argument.

## What is offered

Names harvested from `register_taxonomy('name', ...)` and
`unregister_taxonomy('name')` calls across the project's `.php`
files (skipping `.git`, `node_modules` and `vendor`; at most 500
files, 1 MB each). Textual scan; nothing is executed. Names from
commented-out code, from lookalike functions and from inside strings
are not collected. WordPress core taxonomies (`category`,
`post_tag`, ...) are not listed - only taxonomies the project itself
registers. Taxonomy names inside query arrays (`WP_Tax_Query`, term
functions) are a separate context and not covered here. Unsaved
edits are not seen until the lists cache expires.
