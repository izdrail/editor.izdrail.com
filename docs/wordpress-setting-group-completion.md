# WordPress settings group name completion

Typing a settings group name offers the groups the project itself
registers under (item 430).

## Where it works

- `register_setting('...', $option)` - the group being used.
- `settings_fields('...')` - the group is the first argument.

The second argument of `register_setting($group, '...')` completes
the option name list instead (item 422) - a registered setting is
stored as an option.

## What is offered

Groups harvested from `register_setting('group', ...)` calls across
the project's `.php` files (skipping `.git`, `node_modules` and
`vendor`; at most 500 files, 1 MB each). Textual scan; nothing is
executed. Groups from commented-out code, from lookalike functions
and from inside strings are not collected. WordPress core groups
(`general`, `reading`, `discussion`, ...) are not listed - only
groups the project itself registers under. Unsaved edits are not
seen until the lists cache expires.
