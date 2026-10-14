# Go to Env / Config Key

"Go to Env / Config Key" in the command palette lists the project's settings and opens the line that defines the one you pick.

- `env  APP_NAME  (.env)`: variables from `.env`, `.env.local`, `.env.example`, `.env.dist`. A name defined in several files is listed once, for the first file. Only names are shown, never values.
- `config  app.name  (config/app.php)`: string keys at the first and second level of the array a Laravel `config/*.php` file returns, such as `database.connections`, `database.connections.mysql`.

This complements Go to Definition on `env('X')` and `config('a.b')`, which jump from the code that uses a key. The picker is for browsing when you have no usage open.

Limits: static scan only, nothing is run. Keys built dynamically, config merged from packages, and `.env.*.local` variants beyond the four files are not listed. Third and deeper config levels are not listed. 1 MB per file, 40 config files, 5000 entries, symlinks skipped.
