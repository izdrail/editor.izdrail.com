# PHP backtick strings

The standalone PHP and Laravel scanners treat backtick command strings as quoted strings, with simple `$variable` interpolation and bounded escape handling. Quotes, line breaks and `?>` within the command body stay in that string; code after its closing backtick resumes normal PHP highlighting. Missing closers retain string scope to EOF during edits.

This also prevents PHP-only routing from treating a `?>` in a command body as emitted HTML. The editor does not execute command strings. PHP 8.5 deprecates this operator, but older source still needs correct highlighting.

Source: https://www.php.net/manual/en/language.operators.execution.php . A token_get_all check confirmed `?>` stays within T_ENCAPSED_AND_WHITESPACE.

Curly interpolation beginning with `{$` owns its balanced expression until `}`. Quoted array keys and braces in quoted/commented expression text cannot end the surrounding double-quoted or backtick string. Simple variable spans retain their variable scope. This is bounded lexical highlighting, not full PHP expression parsing or execution; unfinished expressions stay inside the current string snapshot.

Reference: https://www.php.net/manual/en/language.types.string.php .
