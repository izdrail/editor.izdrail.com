# Lexer corpus and regression gate (L9)

Current selection: standard builds default to Logos on supported routes. Settings > Editor > Lexer chooses Logos or Lexilla; an environment override is available. See [the current guide](logos-lexer.md). The phase notes below describe the implementation at that checkpoint.

Eight checked-in fixtures cover standard PHP, modern declarations and PHP8.4 hooks, Laravel conventions, Blade mixed HTML, Symfony/Doctrine attributes, multiline interpolation/heredoc/nowdoc and malformed/incomplete input. Existing focused Blade/Laravel/Symfony and cooperative/incremental tests remain part of the gate.

`tests/support/php-token-oracle.php` reads source on stdin and calls `token_get_all` without TOKEN_PARSE. It never includes, evaluates or executes the source. It emits token names, byte offsets and PHP version. The Rust gate compares PHP-owned comment, variable, number, tag, name and literal-string regions. Interpolated variables may use the separate interpolation scope; open/close tag trailing whitespace is deliberately excluded. Blade syntax is not misrepresented as PHP syntax: the PHP oracle owns only actual PHP regions, while separate Blade tests own directives and echoes.

The oracle is mandatory in CI. It may explicitly skip when no PHP executable exists locally. `PHP_BINARY` and `PHP_TOKENIZER_EXTENSION` allow an isolated local CLI; hosted acceptance uses the installed PHP CLI. PHP version is printed into the captured acceptance log. Tokenization of unsupported/new syntax is allowed without parsing it; the dedicated modern-PHP tests assert its lexical scopes.

The corpus also runs every UTF-8 prefix as rapid-typing incremental updates against a full-lex reference and repeats a fixture into a large input that must yield cooperatively while retaining byte-for-byte styles. Existing 2,000 seeded edit sequences and multiline/cancellation tests are not replaced.

Only serde_json is added as a development dependency, already present in the workspace lockfile. Runtime dependencies are unchanged. Lexilla remains default, and fixtures are data rather than executable application tests.
