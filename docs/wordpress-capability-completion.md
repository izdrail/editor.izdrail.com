# WordPress capability name completion

Typing a capability check offers the capability names the project
itself checks for (item 432).

## Where it works

- `current_user_can('...')` - the capability is the first argument.
- `user_can($user, '...')` and `author_can($post, '...')` - the
  capability is the second argument.

## What is offered

Capabilities harvested from those checks across the project's
`.php` files (skipping `.git`, `node_modules` and `vendor`; at most
500 files, 1 MB each). Textual scan; nothing is executed. A
capability the project never checks is dead weight, so checks alone
declare the list - this naturally covers both custom capabilities
(`shop_manage`) and the core capabilities the project actually
references (`manage_options`). Capabilities from commented-out
code, from lookalike functions and from inside strings are not
collected. Unsaved edits are not seen until the lists cache
expires.
