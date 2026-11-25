# Experimental highlighting backend selection (L7)

Lexilla is the default, including in builds that compile Logos support. There is one backend-selection hook in `lapce-core/src/lexilla.rs`.

To opt in for a developer build:

```
cargo build -p lapce-app --features logos-php
GRAVITY_LEXER=logos target/debug/gravity
```

The existing root package can also enable the dependency feature with `--features lapce-app/logos-php`. `GRAVITY_LEXER=lexilla`, an unset value, an empty value, an unknown value, or a build without the feature all use native routing. Only the exact case-insensitive value `logos` opts in. Restart to apply an environment change; no persistent settings are written.

Supported experimental routes are PHP/HTML, Blade and existing Laravel/Symfony convention profiles. Symfony Twig uses the existing Twig composition with bridge vocabulary, not a new parser. Unsupported languages and WordPress-only PHP fall through to the original native path.

Themes consume the same scope adapter. Completion, diagnostics, formatting, navigation, inference and serialization are not routed here. The native path is preserved. At the selection hook the native snapshot supplies folding levels to the experimental highlighting snapshot. This intentionally still runs the native lexer, avoiding a folding regression until an equivalent experimental fold producer exists. Experimental token scopes affect existing pairing/comment context only through the shared snapshot interface. This opt-in does not claim a speed win or justify changing the default.

Tests call an explicit backend parameter rather than mutating process environment, so parallel test execution cannot leak selection. Hosted acceptance runs the feature integration tests and the ordinary full workspace suite.
