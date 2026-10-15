# Go to `.env` variable

"Go to Definition" with the cursor on an environment variable name opens the `.env` line that
defines it, without a language server. Recognised forms, in any file:

- PHP: `env('APP_NAME', ...)`, `getenv('X')`, `$_ENV['X']`, `$_SERVER['X']`
- Symfony: `%env(DATABASE_URL)%`, `%env(int:PORT)%`, `%env(resolve:X)%`
- JavaScript: `process.env.X`, `import.meta.env.VITE_X`
- `${X}` references inside `.env` files

Files are tried in this order and the first that defines the key wins: `.env`, `.env.local`,
`.env.example`, `.env.dist` (workspace root only, each read up to 1 MB). A line defines a key when it
reads `KEY=...` (optionally `export KEY=...`); `KEY_2=` does not match `KEY`. Only the variable name
counts; if no file has a real `KEY=...` line, the first commented-out `# KEY=...` line is used instead (Laravel's `.env.example` ships many defaults that way, for example `# DB_HOST=127.0.0.1`); the default value argument and unrelated strings do nothing. When the key is defined nowhere,
normal Go to Definition runs. Nothing is executed and values are never shown or copied.

Not included: `.env` key completion, which would go through the completion popup that is in
flight elsewhere, and an env viewer panel (a new panel kind).
