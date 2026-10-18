# Importing VS Code extension content (.vsix)

`lapce --install-vsix FILE...` imports the declarative parts of a VS Code
extension. Extension code (`main` / `browser`) is never loaded or executed;
such extensions are reported as "contains code, not loaded".

Imports are stored per extension under `<data dir>/vsx/<publisher>.<name>/`:

| Content | Handling |
| --- | --- |
| Color themes | VS Code theme JSON (JSONC, `include`, `colors`, `tokenColors`) converted to Gravity color-theme TOML, listed in `volt.toml` so the existing theme loader picks it up |
| Icon themes | SVG icons copied, mapping converted to Gravity icon-theme TOML (file extensions, file names, folder names) |
| TextMate grammars | `.tmLanguage.json` kept verbatim and run by the built-in engine (`lapce-vsix::textmate`, Oniguruma regexes) for files Lexilla has no lexer for |
| Snippets | parsed to a normalised JSON file (`Loaded::snippets_for`) |
| Language configuration | parsed to normalised JSON (`Loaded::language_configs`) |

`vsx.json` in each directory caches what was imported, the licence declared by
the extension, and anything skipped with the reason.

Theme matching: TextMate-highlighted text carries a scope stack as its style
name (innermost first). `LapceConfig::style_color` tries each scope, trimming
dotted suffixes, so a theme rule for `string` colours `string.quoted.double`
as in VS Code. Converted themes also define every key Lexilla emits, so the
default theme does not leak through.

Do not use this to fetch from marketplace.visualstudio.com. Open VSX packages
are fine; check each extension's licence.

## Known gaps

- `fontStyle` (bold/italic), descendant and exclusion theme selectors are ignored.
- Plist (`.tmLanguage`) grammars and font-based icon themes are not imported.
- Icon mappings keyed by multi-dot extension or by `languageIds` are skipped.
- Snippets and language configuration are imported but not yet applied by the editor UI.
- Grammars apply only where Lexilla has no lexer for the file.

## Open VSX marketplace (command line)

Extensions come from the Open VSX registry (https://open-vsx.org) only, never
the Microsoft Marketplace.

```
lapce --ext-search dracula                 # search, 20 hits
lapce --ext-install dracula-theme.theme-dracula
lapce --ext-list                           # what is imported
lapce --ext-uninstall dracula-theme.theme-dracula
```

`--ext-install` fetches the latest version, checks it against the sha256 the
registry publishes, and runs the same import as `--install-vsix`. Only file
URLs on open-vsx.org are followed and downloads are capped at 100 MB. Extensions
with `main`/`browser` code install their declarative content and are marked
"contains code, not loaded". The shim in `lapce-vscode-experiment` is a separate
opt-in experiment and is not used by installs.

## Open VSX marketplace (plugin panel)

The plugin panel has two new sections above "Available Themes & Icons":
"Open VSX Marketplace" (results for the panel's existing search box, with
Install buttons) and "Installed from Open VSX" (imported extensions with
Uninstall). Installing reloads the config so new themes show up in the theme
picker. Each installed row says "Contains code, not loaded" or "Declarative
content only". The existing plugins.lapce.dev lists are unchanged.

Not done yet: update checks, version pinning, paging past the first 30 hits,
extension icons and readmes, enable/disable per extension.
