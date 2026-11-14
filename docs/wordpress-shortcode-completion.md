# WordPress shortcode tag completion

Typing a WordPress shortcode tag offers the tags the project itself
registers (item 420).

## Where it works

- `add_shortcode('...')` - the tag being registered.
- `remove_shortcode('...')` and `shortcode_exists('...')` - the tag is
  the first argument.
- `has_shortcode($content, '...')` - the tag is the second argument.
- `do_shortcode('[ta` / `do_shortcode('[/ta` - inside the tag name of
  the bracket string; past the tag name (after a space or `]`) the
  list stays silent.

## What is offered

Tags harvested from `add_shortcode('tag', ...)` registrations across
the project's `.php` files (skipping `.git`, `node_modules` and
`vendor`; at most 500 files, 1 MB each). Textual scan; nothing is
executed. Names from commented-out code, from lookalike functions and
from inside strings are not collected. WordPress core shortcodes are
not listed - only tags the project itself registers. Unsaved edits
are not seen until the lists cache expires.

Go to Definition on shortcode tags and callbacks is separate
(see go-to-wordpress-hook).
