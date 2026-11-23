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
