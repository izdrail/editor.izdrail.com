# Heredoc and nowdoc opener spacing

The PHP and Laravel scanners accept spaces and tabs between `<<<` and an unquoted, double-quoted or single-quoted label. The same rule applies when recovering an existing multiline string. Newlines are not skipped as label spacing.

This preserves string scope for the body and PHP scope after the closing label. A `?>` in such a body no longer makes PHP-only routing mistake it for emitted HTML. Indentation validation and complex interpolation remain outside this small lexical fix.

Source: https://www.php.net/manual/en/language.types.string.php . Spaces and tabs were also checked against PHP token_get_all with TOKEN_PARSE.

Heredoc and nowdoc opening lines and body lines accept CR, LF and CRLF endings. CRLF is consumed as one line boundary. Closing-label checks still run at each logical line start, so code after a CR-only closing label resumes normal PHP highlighting. No line endings are rewritten by the lexer.
