# Experimental highlighting backend selection (L7)

Lexilla is the default, including in builds that compile Logos support. There is one backend-selection hook in `lapce-core/src/lexilla.rs`.

To opt in for a developer build:

```
cargo build --locked --bin gravity --features lapce-app/logos-php
GRAVITY_LEXER=logos target/debug/gravity
```

The existing root package can also enable the dependency feature with `--features lapce-app/logos-php`. `GRAVITY_LEXER=lexilla`, an unset value, an empty value, an unknown value, or a build without the feature all use native routing. Only the exact case-insensitive value `logos` opts in. Restart to apply an environment change; no persistent settings are written.

Supported experimental routes are PHP/HTML, Blade and existing Laravel/Symfony convention profiles. Symfony Twig uses the existing Twig composition with bridge vocabulary, not a new parser. Unsupported languages and WordPress-only PHP fall through to the original native path.

Themes consume the same scope adapter. Completion, diagnostics, formatting, navigation, inference and serialization are not routed here. The native path is preserved. For the non-incremental highlighting API, the native snapshot supplies folding levels in the same request. The document incremental path instead uses a separate 250 ms-debounced native-fold refresh, accepting only same-revision folds. See [the current architecture guide](logos-lexer.md) and [incremental design](logos-incremental.md). Experimental token scopes affect existing pairing/comment context only through the shared snapshot interface. This opt-in does not claim a speed win or justify changing the default.

Tests call an explicit backend parameter rather than mutating process environment, so parallel test execution cannot leak selection. Hosted acceptance runs the feature integration tests and the ordinary full workspace suite.
