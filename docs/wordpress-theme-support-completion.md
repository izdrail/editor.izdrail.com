# WordPress theme support completion

Typing the first argument of a theme support call offers feature
names: the custom features the project itself declares plus the
core features (item 441).

## Where it works

The feature is the first argument of `add_theme_support(...)`,
`remove_theme_support(...)`, `get_theme_support(...)` and
`current_theme_supports(...)`.

## What is offered

Feature names from the first quoted argument of those calls across
the project's `.php` files (skipping `.git`, `node_modules` and
`vendor`; at most 500 files, 1 MB each), plus the core features
themes usually declare (`post-thumbnails`, `html5`,
`custom-logo`, ...). Textual scan; nothing is executed. Features
from commented-out code, from lookalike functions and from inside
strings are not collected. Unsaved edits are not seen until the
lists cache expires.
