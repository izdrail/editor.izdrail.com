# Highlighting backend selection

Standard builds include Logos and select it by default for supported PHP/Blade
and Laravel/Symfony routes. This changed by explicit product choice after the
[benchmark gate](logos-benchmarks.md), not a measured overall speed win.

Settings > Editor > Lexer stores `logos` or `lexilla` in `editor.lexer`. It
applies on the next edit or file reopen. `GRAVITY_LEXER=lexilla` is a process-wide
native override; `logos` forces Logos. Unset/empty environment values use the
saved setting. Unknown nonempty overrides use native fallback. Custom builds
without `logos-php` always use native routing.

The single backend hook remains in `lapce-core/src/lexilla.rs`. Unsupported
languages and WordPress-only PHP stay on native/composed routing. Symfony Twig
reuses the existing composition with bridge vocabulary. The non-incremental
API preserves native folds in the same call; document colouring uses a separate
250 ms-debounced, revision-gated native-fold refresh. Both native folding paths
explicitly request Lexilla, independently of the default or Settings choice.

See [the architecture and settings guide](logos-lexer.md) and
[incremental limits](logos-incremental.md). Completion, diagnostics, formatting,
navigation, inference and serialization remain separate. No speed guarantee.
