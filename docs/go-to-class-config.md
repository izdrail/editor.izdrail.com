# Go to a class named in Symfony config

"Go to Definition" in `.yaml`, `.yml`, `.xml`, `.twig` and `.neon` files opens the PHP class under the
cursor. Recognised: a fully qualified name (at least one `\`, starting with a capital) such as
`App\Service\Mailer`, `'@App\Service\Mailer'`, `class: App\X::class`, `<service id="App\\Foo"/>`
(doubled backslashes are read as one), and `App\Controller\Home::index`, which opens the `index` method.

The class file comes from the PSR-4 entries in `composer.json` (`autoload` and `autoload-dev`, several
directories per prefix allowed, longest prefix wins); with none, `App\` maps to `src/`. The cursor lands
on the `class`/`interface`/`trait`/`enum` line (or the top of the file if it is not found). Names with `..`,
unknown namespaces and classes that do not exist on disk do nothing, and normal Go to Definition runs.
PHP files themselves are left to the language server. Classics such as `classmap`/PSR-0 autoloading and
vendor classes are not resolved. Nothing is executed; files over 1 MB are not read for the line.
