# Go to Composer package

In `composer.json`, Go to Definition (Ctrl/Cmd-click or F12) on a `"vendor/package"` key in `require`, `require-dev`, `suggest`, `conflict` and similar sections opens that package's own `composer.json` under `vendor/`, where its autoload map and requirements are. In `composer.lock` the same works on the `"name": "vendor/package"` value.

Nothing is run. Platform requirements (`php`, `ext-json`), version strings, the project's own `"name"` and packages that are not installed do nothing (the normal Go to Definition runs instead). A package that is a symlink pointing outside `vendor/` (a path repository) is not followed. Package names are checked (`lowercase/lowercase`, no `..`).
