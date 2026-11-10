# JSON with Comments config files

A distinct JSONC mode highlights JavaScript-style line and block comments in documented config paths. It does not enable JSON5 single-quoted strings or unquoted property names. Ordinary `package.json`, `composer.json` and generic `settings.json` files remain strict JSON.

Automatic routing covers `tsconfig.json`, `jsconfig.json`, `.eslintrc.json`, VS Code config names inside `.vscode`, `.code-workspace` files, and documented devcontainer config locations. Other JSON filenames are unchanged. JSONC has a `//` toggle-comment token; strict JSON remains a no-op.

This is lexical highlighting, not a JSON schema validator or complete parser. Trailing commas are not used as proof of validation.

Sources rechecked: https://code.visualstudio.com/docs/languages/json and https://containers.dev/implementors/spec/
