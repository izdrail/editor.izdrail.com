# Env variable hover

Hovering an environment-variable reference shows where the variable is
defined and the code default, without ever showing the value (matching the
env palette index, which lists names only):

    env('APP_NAME', 'Laravel')   -> **`APP_NAME`** - set in `.env`
                                    .env:1
                                    Code default: `Laravel`.
    env('MAIL_FROM')             -> **`MAIL_FROM`** - commented out in `.env.example`
    env('NOPE')                  -> **`NOPE`** - not defined in any .env file

Recognised references (same set as the env go-to definition):

- PHP: `env('X')`, `getenv('X')`, `$_ENV['X']`, `$_SERVER['X']`
- Symfony YAML/PHP: `%env(X)%`, `%env(int:PORT)%` and other processors
- JavaScript/TypeScript: `process.env.X`, `import.meta.env.X`
- `.env` files themselves: `${X}` interpolation

Files are tried in order `.env`, `.env.local`, `.env.example`, `.env.dist`;
the first definition wins. A definition found only as a commented-out
`# KEY=value` line is reported as "commented out". When `env('X', default)`
has a second argument it is shown as the code default (quoted strings and
bare scalars like `false` or `null`).

Nothing is executed; files are read as text, capped at 1 MB. Works in any
file type, not just PHP.
