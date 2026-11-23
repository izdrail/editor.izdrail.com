# Smart editing controls

The existing Editor settings now include separate controls for automatic bracket pairing, quote pairing and moving over a closing delimiter. They use the existing settings storage and are configurable in the settings UI or settings.toml:

```toml
[editor]
auto-closing-matching-pairs = true
auto-close-brackets = true
auto-close-quotes = true
skip-existing-closing = true
auto-surround = true
```

The existing `auto-closing-matching-pairs` option remains a master switch for automatic pairing and skipping. Turning it off disables all three, even if their individual options remain enabled. Selection wrapping remains independently controlled by `auto-surround`. Brackets are parentheses, square brackets and curly brackets. Quotes include single quotes, double quotes and backticks.

All new controls default to enabled, including when loading an older settings file. They apply through the same insertion action in ordinary editors and source-mapped excerpt editors. Multi-character inserts retain literal paste behavior. Existing callers of the core insertion API retain their previous defaults.

This is the first granular settings slice, not a finished smart-editing settings page or language-context policy. Context-aware pairing, separate Smart Enter/indent controls, language-specific overrides and dedicated settings organization remain open.
