# WordPress sidebar / widget area id completion

Typing a sidebar id offers the widget areas the project itself
declares or uses (item 434).

## Where it works

- `dynamic_sidebar('...')` - the sidebar id is the first argument.
- `is_active_sidebar('...')` - same.

## What is offered

Sidebar ids harvested across the project's `.php` files (skipping
`.git`, `node_modules` and `vendor`; at most 500 files, 1 MB each)
from two sources: the `'id' =>` values of `register_sidebar`
declarations, and ids passed to `dynamic_sidebar` /
`is_active_sidebar` - a used id is worth completing even where its
declaration is not visible. Textual scan; nothing is executed. Ids
from commented-out code, from lookalike functions and from inside
strings are not collected. WordPress core sidebars are not listed -
only ids the project itself declares or uses. Unsaved edits are not
seen until the lists cache expires.
