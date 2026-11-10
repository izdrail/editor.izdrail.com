# PHP base word casing

The standalone PHP and Laravel scanners match their base keyword, function and type lists with ASCII case folding. Uppercase and mixed-case spellings such as `IF`, `ReTuRn`, `STRLEN`, `TRUE` and `InT` keep the same scope as their lowercase form. Supplied function/type lists are normalized too, including project metadata with mixed-case names.

Variables, strings and comments are unchanged. The fourth word list (ordinary constants) and Laravel-specific lists retain exact matching. This change does not add semantic symbol resolution or expand the shipped API vocabulary.

Sources: https://www.php.net/manual/en/functions.user-defined.php and https://www.php.net/manual/en/language.constants.syntax.php . Keyword behavior was also checked with PHP token_get_all.
