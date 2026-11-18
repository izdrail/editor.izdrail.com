# PHP property-hook magic constant

The PHP and Laravel standalone lexers recognize `__PROPERTY__`, including mixed-case spellings, as a magic constant. The offline PHP completion list offers `__PROPERTY__` with constant kind and exact insertion text. Global fallback suggestions remain suppressed in strings, comments and object-member contexts.

PHP introduced this magic constant for property hooks in PHP8.4. This change is lexical coloring and an offline completion entry, not runtime-version detection or property-hook type inference. Existing project language servers still provide richer PHP intelligence.

Reference: https://www.php.net/manual/en/language.constants.magic.php
