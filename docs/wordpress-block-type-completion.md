# WordPress block type name completion

Typing a block name offers the block types the project itself
registers (item 437).

## Where it works

- `register_block_type('...')` - the string form names the block
  first. The metadata-path form
  (`register_block_type(__DIR__ . '/build')`) opens with an
  expression and completes nothing.
- `unregister_block_type('...')` - same.

## What is offered

Block names harvested from those calls across the project's
`.php` files (skipping `.git`, `node_modules` and `vendor`; at
most 500 files, 1 MB each). Textual scan; nothing is executed.
Names from commented-out code, from lookalike functions and from
inside strings are not collected. Core block names
(`core/paragraph`, ...) are not listed - only names the project
itself registers. Unsaved edits are not seen until the lists
cache expires.
