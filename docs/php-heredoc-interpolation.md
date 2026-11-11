# Heredoc variable scopes

The PHP and Laravel scanners give simple `$name` spans in heredoc bodies the same variable scope as double-quoted strings. Escaped dollars remain string text; an even pair of backslashes permits a following variable. Nowdoc bodies remain literal, including dollar signs and braces. Closing-label detection still runs at the start of each line, including after a trailing backslash.

This is lexical highlighting, not expression evaluation. A simple variable inside braces is colored, but the scanner does not parse arbitrary nested interpolation expressions. Source: https://www.php.net/manual/en/language.types.string.php .

PHP and Laravel/Blade now share their heredoc/nowdoc opener, body and resume scanners through `PHPHeredocScanner.h`. Each lexer supplies its existing style IDs. A parity corpus covers quote modes, horizontal opener whitespace, UTF-8, LF/CRLF/CR and unfinished source. This extraction preserves existing lexical limits; it does not add PHP compiler-level indentation validation.
