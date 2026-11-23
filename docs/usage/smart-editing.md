# Smart editing controls

The existing Editor settings now include separate controls for automatic bracket pairing, quote pairing and moving over a closing delimiter. They use the existing settings storage and are configurable in the settings UI or settings.toml:

```toml
[editor]
auto-closing-matching-pairs = true
auto-close-brackets = true
auto-close-quotes = true
skip-existing-closing = true
auto-surround = true
smart-enter = true
smart-indent = true
```

The existing `auto-closing-matching-pairs` option remains a master switch for automatic pairing and skipping. Turning it off disables all three, even if their individual options remain enabled. Selection wrapping remains independently controlled by `auto-surround`. Brackets are parentheses, square brackets and curly brackets. Quotes include single quotes, double quotes and backticks.

All new controls default to enabled, including when loading an older settings file. They apply through the same insertion action in ordinary editors and source-mapped excerpt editors. Multi-character inserts retain literal paste behavior. Existing callers of the core insertion API retain their previous defaults.

This is the first granular settings slice, not a finished smart-editing settings page or language-context policy. Context-aware pairing, language-specific overrides and dedicated settings organization remain open.

`smart-enter = false` stops structural Enter expansion while retaining line indentation if `smart-indent` is enabled. `smart-indent = false` inserts a plain line break without preserving or adding indentation. These switches apply in ordinary and source-mapped excerpt editors. Defaults preserve existing behavior.

Quotes and backticks immediately after an odd-length backslash run insert literally, without pairing or skipping. Even runs retain normal pairing/skipping. Explicit selected ranges are always replaced when selection wrapping is disabled; closer skipping only applies to carets. This is a conservative core fix, not complete language-aware string/comment pairing.

Partner insertion now consults the existing current-revision lexer snapshot separately at every caret, including canonical source offsets in excerpt editors. Comment regions and string interiors receive only the character typed, not an added partner. Missing, stale or wrong-length snapshots also fall back to literal insertion, without lexing the whole document synchronously on a keystroke. Existing-closer skipping and explicit selection wrapping are independent of this filter.

The verified snapshot cases cover PHP, Python, Rust, JavaScript, TypeScript, CSS, HTML, Blade and Vue. The current Lexilla adapter has no Go route, so Go receives conservative literal insertion until that existing language-highlighting gap is addressed. This is not complete language-specific quoting: raw strings, Rust lifetimes and complex interpolation still require dedicated acceptance.

Bracket highlighting now reaches the existing structural fallback when the current syntax state has no tree layers. The fallback checks a bracket directly under the caret before its enclosing pair, so opening brackets are included. This fallback is language-unaware and does not claim string/comment filtering or unmatched-bracket feedback.

Structural matching also checks the character immediately before the caret, including after a closing bracket at end of file. A bracket under the caret takes precedence over the previous character for touching pairs such as `()[]`.

The existing Match Pairs command also accepts a bracket immediately before the caret in the structural fallback. It does not jump from arbitrary code to an enclosing bracket. Existing configurable shortcuts are unchanged.
