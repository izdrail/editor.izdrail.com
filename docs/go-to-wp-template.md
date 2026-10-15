# Go to WordPress template / include

Go to Definition with the cursor on the string opens the file:

- `get_template_part('template-parts/content', 'page')` opens `template-parts/content-page.php`, else `template-parts/content.php`. With one argument it opens `<slug>.php`.
- `get_header('shop')`, `get_footer(...)`, `get_sidebar(...)` open `header-shop.php`, else `header.php` (and so on).
- `locate_template('inc/x.php')`, `get_theme_file_path('/inc/x.php')`, `get_parent_theme_file_path(...)`.
- `get_template_directory() . '/inc/x.php'` and `get_stylesheet_directory() . ...` (the theme folder).
- `__DIR__ . '/x.php'`, `plugin_dir_path( __FILE__ ) . 'x.php'`, `dirname(__FILE__) . '/x.php'` (the folder of the current file).

The theme folder is the nearest folder above the current file with a `style.css`, else the workspace root. Textual only, nothing is run. Child-theme overrides, `get_template_part` arguments passed as variables, `..` and absolute paths are not handled; symlinked files are not followed and the file must be under the workspace.

Status: unit tests cover each form, fallbacks, traversal, symlinks and random-input fuzz. The GUI jump is not yet verified.
