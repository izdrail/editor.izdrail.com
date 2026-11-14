# WordPress meta key completion

Typing a meta key offers the keys the project itself uses (item
431).

## Where it works

- `get_post_meta($id, '...')` and the add/update/delete variants
  for posts, users, terms and comments - the key is the second
  argument.
- `get_metadata('post', $id, '...')` and the add/update/delete
  variants - the key is the third argument.

## What is offered

Keys harvested from those calls across the project's `.php` files
(skipping `.git`, `node_modules` and `vendor`; at most 500 files,
1 MB each). Textual scan; nothing is executed. Reads declare keys
just like writes - a key read anywhere is worth completing
elsewhere. Keys from commented-out code, from lookalike functions
and from inside strings are not collected. The scan expects the
common literal forms - an unquoted id in the object-specific calls
and a quoted meta type in the generic `*metadata` calls; other
shapes (a variable meta type, a quoted id, a variable key) are
approximated or missed. WordPress core keys are not listed - only
keys the project itself uses. Unsaved edits are not seen until the
lists cache expires.
