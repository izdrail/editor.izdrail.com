# Framework symbol hover

Hovering a framework API name shows its signature, the pack it came
from and a documentation link (item 442).

## Where it works

In `.php`, `.blade.php` and `.twig` files, for names in the
generated offline pack (`metadata/web/symbols.json`): the PHP
standard library everywhere, WordPress and Symfony symbols when the
detected project profile says so (a `wp-content` directory, a
plugin or theme header, the matching Composer package), Twig tags,
filters, functions and tests in `.twig` files.

## What is shown

The declared signature (when the pack carries one), the symbol kind,
the package and version baseline it was generated from, and the
documentation URL. WordPress and Twig links point at the reference
index, not a per-symbol page. The URL renders as link-styled text;
markdown links are not clickable yet.

Static lookup against the generated pack; nothing is executed. The
hover sits after the project-specific providers (routes, config,
env, validation, artisan, middleware) and before the language
server fallback.
