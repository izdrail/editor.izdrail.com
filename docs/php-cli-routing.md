# PHP framework entry scripts

Extensionless `artisan` and `bin/console` files now select PHP syntax and send the PHP language ID to the language server. A bare `console` or `scripts/console` remains unchanged. This is conventional exact-name routing, not content authentication; a non-PHP file deliberately given one of these names still gets the conventional mode.

A shebang line before the PHP open tag no longer diverts PHP CLI source to the mixed HTML lexer. CRLF shebangs are covered. Non-PHP text and HTML after a shebang do not count as pure PHP.

Source examples checked: https://github.com/laravel/laravel/blob/12.x/artisan and https://github.com/symfony/recipes/blob/main/symfony/console/5.3/bin/console

Project completion follows the same PHP/Blade path routing. In `artisan` and `bin/console`, the popup can offer project names inside helper calls, including on an empty opening quote. Accepting `reports:send` over `reports:s` or `app.name` over `app.na` replaces the whole name, rather than adding a duplicate prefix. Bare `console`, `scripts/console` and other unknown extensionless files keep the non-PHP gate.

Go to Definition also uses PHP path routing for local `view(...)`, Twig `render(...)` and `asset(...)` references in these CLI scripts. It keeps the existing bounded static resolver and path checks, without running PHP or requiring a language server. Unknown extensionless `console` files do not gain these lookups.
