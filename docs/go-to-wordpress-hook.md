# Go to WordPress hook or callback

"Go to Definition" in a `.php` file understands WordPress hooks, without a language server:

- On a callback: `add_action('init', 'my_cb')`, `add_filter('x', [$this, 'run'])`,
  `array(__CLASS__, 'boot')`, `'Cls::run'`, `add_shortcode('tag', 'cb')`,
  `register_activation_hook(__FILE__, 'cb')` (also deactivation/uninstall): opens `function my_cb(`.
  For `[$obj, 'method']` and `Cls::method` it looks for `function method(` by name, so a method with
  the same name in another class can win.
- On a hook name in `add_action` / `add_filter`: opens the first `do_action` / `apply_filters`
  (also `_ref_array`) that fires it.
- On a hook name in `do_action` / `apply_filters`: opens the first `add_action` / `add_filter`
  that registers it.
- On a shortcode tag inside `do_shortcode('[gallery id="1"]')` (also `[/gallery]`), or the name in
  `shortcode_exists('gallery')` / `remove_shortcode('gallery')` / `has_shortcode($content, 'gallery')`:
  opens the first `add_shortcode('gallery', ...)`. Shortcodes written in post content or
  `.html` templates (outside a PHP string) are not scanned.

Only the hook-name and callback arguments count (not priority or arg count), and quoted names only.

Search limits: the current file first, then `.php` files under the workspace root in directory order,
skipping `.git`, `node_modules` and `vendor`; at most 4000 files, 1 MB each, about 0.4 s of walking
(0.8 s total). It reads files from disk, so unsaved edits are not seen, and on a very large tree an
answer may not be found. This runs on the editor thread, so a worst case is a sub-second pause. If
nothing is found, normal Go to Definition runs. Nothing is executed.

Not included: hooks registered with variable names or constants, closures, and the WordPress core
hook list.

## Post types, taxonomies, script handles and options

On the name in one of these uses, Go to Definition opens the first matching registration, found the same
way as hooks:

| On | Opens |
| --- | --- |
| `'post_type' => 'book'`, `is_singular('book')`, `is_post_type_archive(...)`, `get_post_type_object(...)` | `register_post_type('book'` |
| `'taxonomy' => 'genre'`, `is_tax(...)`, `get_taxonomy(...)` | `register_taxonomy('genre'` |
| `wp_enqueue_script('h')`, `wp_dequeue_script`, `wp_script_is` | `wp_register_script('h'` |
| `wp_enqueue_style('h')` and friends | `wp_register_style('h'` |
| `get_option('x')`, `delete_option('x')` | the first `add_option('x'` or `update_option('x'` |

Names built at runtime, `'post_type' => ['a', 'b']` arrays, `register_setting()` and handles that are
only enqueued with a source (no separate register call) are not found. Nothing is executed.
