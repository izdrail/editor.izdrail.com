# PHP-first mixed templates

A PHP file can start with `<?php` and later leave PHP to emit HTML, CSS or JavaScript. These templates now use the existing mixed-web lexer instead of treating the emitted markup as one plain span. PHP-only source, including an optional final close tag followed only by whitespace, keeps the standalone PHP/Laravel lexer.

Routing checks the existing PHP lexer's close-tag and inline-HTML styles. A `?>` inside a quoted string, block comment, heredoc or nowdoc does not turn source into a template. Line-comment close tags follow the PHP lexer's existing behavior. Files without a `?>` substring keep the cheap prefix-only check.

This does not add a new scanner or change the PHP lexer itself. Mixed-web script/style delegation and regional comment rules remain the existing bounded behavior.

Sources checked: https://www.php.net/manual/en/language.basic-syntax.phptags.php and https://www.php.net/manual/en/language.basic-syntax.comments.php
