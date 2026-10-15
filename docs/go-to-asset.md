# Go to Asset

Go to Definition with the cursor on the string opens the file, in Blade, Twig, PHP and HTML files:

- `asset('css/app.css')`, `secure_asset(...)`, `mix('/js/app.js')`, `Vite::asset('resources/img/x.png')`, `asset_path(...)`: looked up in `public/`, then `assets/` (Symfony AssetMapper), then `web/`. A `?v=1` query or `#hash` is ignored.
- `@vite('resources/js/app.js')` and `@vite(['resources/css/app.css', 'resources/js/app.js'])`: relative to the project root.

Textual only, nothing is run. `..`, absolute paths, URLs and strings with variables are refused; symlinked files are not followed and the file must be under the workspace. Vite aliases (`@/...`), Symfony's `asset('x', 'package')` package paths and the Webpack Encore manifest (`entrypoints.json`) are not read.

Status: unit tests cover each call form, refusals, symlinks and random-input fuzz. The GUI jump is not yet verified.
