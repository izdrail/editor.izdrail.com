# WordPress template part completion

Typing a `get_template_part` slug offers the template files the
project's themes actually ship (item 433).

## Where it works

- `get_template_part('...')` - the slug is the first argument.

## What is offered

Slugs harvested by walking every theme under `wp-content/themes`
(bounded depth, symlinks not followed): each `.php` file becomes
its theme-relative path without the extension, so
`wp-content/themes/shop/template-parts/shop.php` completes as
`template-parts/shop`. Several themes providing the same file
produce one entry. Nothing is executed. Unsaved edits and files
added after the lists cache was built are not seen until it
expires.
