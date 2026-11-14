# WordPress user role name completion

Typing a user role name offers the roles the project itself adds
(item 428).

## Where it works

- `add_role('...', ...)` - the role being registered.
- `remove_role('...')` and `get_role('...')` - the name is the first
  argument.

## What is offered

Names harvested from `add_role('name', ...)` and
`remove_role('name')` calls across the project's `.php` files
(skipping `.git`, `node_modules` and `vendor`; at most 500 files,
1 MB each). Textual scan; nothing is executed. Names from
commented-out code, from lookalike functions and from inside strings
are not collected. WordPress core roles (`administrator`, `editor`,
`author`, `contributor`, `subscriber`) are not listed - only roles
the project itself adds. Unsaved edits are not seen until the lists
cache expires.
