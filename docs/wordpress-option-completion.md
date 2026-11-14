# WordPress option and transient name completion

Typing an option or transient name offers the names the project
itself uses (item 422).

## Where it works

- The options API: `get_option('...')`, `add_option(...)`,
  `update_option(...)`, `delete_option(...)` and the `*_site_option`
  network variants.
- The transients API: `get_transient('...')`, `set_transient(...)`,
  `delete_transient(...)` and the `*_site_transient` variants.

In every case the name is the first argument.

## What is offered

Names harvested from all of the calls above across the project's
`.php` files (skipping `.git`, `node_modules` and `vendor`; at most
500 files, 1 MB each). Textual scan; nothing is executed. Names from
commented-out code, from lookalike functions and from inside strings
are not collected. WordPress core option names are not listed - only
names the project itself reads or writes. Unsaved edits are not seen
until the lists cache expires.
