# Go to a PHP class from `X::class`

"Go to Definition" on the class name in a `Post::class` constant in a PHP file opens the
conventional `Post.php` - Laravel folders first (`app/Models`, `app`, `app/Http/Controllers`,
`app/Http/Requests`, `app/Http/Resources`, `app/Http/Middleware`, `app/Jobs`, `app/Events`,
`app/Listeners`, `app/Observers`, `app/Notifications`, `app/Mail`, `app/Policies`,
`app/Providers`, `app/Console/Commands`, `app/Services`, `app/Actions`, `app/Exceptions`),
then Symfony ones (`src/Controller`, `src/Entity`, `src/Service`, `src/Repository`, `src/Form`,
`src/Command`, `src/EventListener`, `src/EventSubscriber`, `src/Security`, `src/Twig`,
`src/Message`, `src/MessageHandler`, `src`), then `database/seeders`/`database/seeds`/
`database/factories` and `tests`, `tests/Feature`, `tests/Unit`. A fully qualified
`\App\Models\Post::class` works the same; the last segment drives the lookup.

The jump is only made when exactly one conventional `Post.php` exists: zero or several
candidates do nothing and normal Go to Definition runs. The cursor lands on the
`class`/`interface`/`trait`/`enum` line (top of the file when the declaration is not found).
`self::class`, `static::class`, lowercase names and `Post::className` are skipped. More
specific resolvers (controller actions, seeders, factories) run first and keep their own
behaviour. Static text scan only; no PHP is run. Limit: 1 MB per file.

Status: unit tests cover the conventional folders, uniqueness, the negative shapes,
declaration kinds, the 1 MB cap and non-ASCII fuzz. The GUI jump is not yet verified.
