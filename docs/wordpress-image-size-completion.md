# WordPress image size name completion

Typing an image size name offers the sizes the project itself
registers (item 424).

## Where it works

- `the_post_thumbnail('...')` - the size is the first argument.
- `get_the_post_thumbnail($post, '...')` - the size is the second
  argument.
- `add_image_size('...', ...)` and `remove_image_size('...')` - the
  name being registered or removed.

## What is offered

Sizes harvested from `add_image_size('name', ...)` registrations
across the project's `.php` files (skipping `.git`, `node_modules`
and `vendor`; at most 500 files, 1 MB each). Textual scan; nothing
is executed. Names from commented-out code, from lookalike functions
and from inside strings are not collected. WordPress core sizes
(`thumbnail`, `medium`, `large`, `full`, `post-thumbnail`) are not
listed - only sizes the project itself registers. Unsaved edits are
not seen until the lists cache expires.
