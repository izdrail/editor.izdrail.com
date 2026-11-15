# WordPress text domain completion

Typing the domain argument of an i18n call offers the text domains
the project itself uses (item 440).

## Where it works

- `__('text', '...')`, `_e(...)`, `esc_html__(...)`,
  `esc_attr__(...)`, `esc_html_e(...)`, `esc_attr_e(...)` - the
  domain is the second argument.
- `_x('text', 'context', '...')`, `esc_html_x(...)`,
  `esc_attr_x(...)` - third.
- `_n('one', 'many', $n, '...')` - fourth.
- `_nx('one', 'many', $n, 'context', '...')` - fifth.
- `load_plugin_textdomain('...')`, `load_theme_textdomain(...)`,
  `load_child_theme_textdomain(...)` - first.

## What is offered

Domains harvested from those calls across the project's `.php`
files (skipping `.git`, `node_modules` and `vendor`; at most 500
files, 1 MB each). Textual scan; nothing is executed. Calls whose
text argument is an expression (`__($text, 'shop')`) are missed
rather than mis-harvested. Domains from commented-out code, from
lookalike functions and from inside strings are not collected.
Unsaved edits are not seen until the lists cache expires.
