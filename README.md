<h1 align="center">
  <img src="extra/images/logo.png" width=200 height=200/><br>
  editor.izdrail.com
</h1>

<h4 align="center">A fast Rust code editor with VS Code content import</h4>

<div align="center">
  <a href="https://github.com/izdrail/editor.izdrail.com/actions/workflows/linux-build.yml" target="_blank">
    <img src="https://github.com/izdrail/editor.izdrail.com/actions/workflows/linux-build.yml/badge.svg" />
  </a>
</div>
<br/>

editor.izdrail.com is a native code editor written in Rust with a [Floem](https://github.com/lapce/floem) UI and [wgpu](https://github.com/gfx-rs/wgpu) rendering. It is a derivative of [Lapce](https://github.com/lapce/lapce) (Apache License 2.0, copyright the Lapce contributors), with its own highlighting engine, language server client, WASI plugin host, VS Code extension import and keymap presets. Debugging (DAP) is not part of this editor.

![](extra/images/screenshot.png)

## Features

### Editing
* Modal (Vim-like) editing, toggleable with `Core > Modal`
* Rope-based buffer, built-in terminal, git and search panels, remote workspaces
* Debug mode (`--debug` or `LAPCE_DEBUG=1`): verbose logs, crash reports with backtraces and language server crash details ([docs/debugging.md](docs/debugging.md))
* Local code folding (groundwork, see [docs/local-folding.md](docs/local-folding.md))
* Syntax highlighting with Lexilla ([docs/lexilla-highlighting.md](docs/lexilla-highlighting.md)), including dedicated PHP and Laravel lexers ([docs/php-lexers.md](docs/php-lexers.md)), plus TextMate grammars imported from `.vsix` files for languages Lexilla has no lexer for

### Language server client
Independent stdio LSP client: completion (with resolve), hover, go to definition and diagnostics. A global `language-servers.toml` registry seeds TypeScript/JavaScript, Python, HTML, CSS and JSON (npm-managed) and Rust, Go and PHP (command-managed, must be installed), plus an opt-in, Laravel-gated laravel-lsp entry with a managed, hash-verified binary download. Custom servers are supported. See [docs/language-servers.md](docs/language-servers.md).

### Executable plugins (WASI)
Lapce-style WASI plugins run in the proxy's plugin catalog: they can provide language features (completion, hover, code actions, formatting, ...), spawn their own language servers and react to editor events. Editor requests fan out to both the built-in language server client and plugins. See [docs/wasi-plugins.md](docs/wasi-plugins.md).

### Snippet packs
Laravel, WordPress, Symfony and Twig snippets, offered by project type. See [docs/snippet-packs.md](docs/snippet-packs.md).

### Project scripts
Run and Debug lists `npm`/`composer`/`artisan` scripts from your project. See [docs/project-scripts.md](docs/project-scripts.md).

### Prettier formatting
Projects with a Prettier config and binary are formatted through it on save. See [docs/format-on-save.md](docs/format-on-save.md).

### Open in Browser
"Open in Browser" in the command palette shows the active HTML/SVG file in your browser. See [docs/open-in-browser.md](docs/open-in-browser.md).

### Auto-close tags
Typing `>` after an opening tag in HTML, Twig, Blade and Vue files inserts the closing tag, and Enter after a Blade `@if`/`@foreach`/... adds the `@end...` line (`auto-close-tags` setting). See [docs/auto-close-tags.md](docs/auto-close-tags.md).

### Colour swatches
A colour square before `#rrggbb`, `rgb()` and `hsl()` literals in CSS, templates and scripts (`enable-color-swatches` setting). See [docs/color-swatches.md](docs/color-swatches.md).
### Blade prop go-to
Go to Definition on an attribute of `<x-...>` opens the component's `@props` entry or constructor parameter. See [docs/blade-props-goto.md](docs/blade-props-goto.md).

### Insert @props Draft
In a component view, inserts an `@props([...])` block from the attributes other views pass to it. See [docs/props-draft.md](docs/props-draft.md).

### Blade hover help
Hover a Blade directive or an `<x-...>` attribute for its syntax or prop declaration. See [docs/blade-hover.md](docs/blade-hover.md).

### Rename Tag
Palette command that renames the opening and closing tag of the element around the caret. See [docs/rename-tag.md](docs/rename-tag.md).

### Wrap Selection in Tag
Palette command that wraps the selection in an HTML, Blade or Livewire element from a list. See [docs/wrap-in-tag.md](docs/wrap-in-tag.md).

### Organize PHP Imports
Palette command that removes unused `use` lines and sorts the rest. See [docs/organize-imports.md](docs/organize-imports.md).
### Sort Tailwind Classes
Palette command that sorts static `class`/`className` utilities into Tailwind's usual order (approximation). See [docs/tailwind-sort.md](docs/tailwind-sort.md).

### Import PHP Class
Palette command that adds a sorted `use` line for the class under the cursor, from composer.json PSR-4 paths and vendor packages. See [docs/import-class.md](docs/import-class.md).

### Go to Composer package
Go to Definition on a package in `composer.json`/`composer.lock` opens its `composer.json` under `vendor/`. See [docs/composer-go-to-package.md](docs/composer-go-to-package.md).
### Go to Test
Palette list of PHPUnit and Pest test files and cases under `tests/`. See [docs/go-to-test.md](docs/go-to-test.md).
### Go to Migration
Palette list of Laravel migrations, newest first, with the tables they touch. See [docs/go-to-migration.md](docs/go-to-migration.md).
### Go to Policy / Gate
Palette list of Laravel policies with their abilities, and `Gate::define` names. See [docs/go-to-policy.md](docs/go-to-policy.md).

### Find Component Usages
Palette list of every Blade view that renders the component open in the editor (`<x-...>`, `<livewire:...>`, `@livewire`). See [docs/component-usages.md](docs/component-usages.md).

### Go to Blade Layout / Section
From a view, lists its `@extends` layout and the layout's `@yield`/`@stack` slots; from a layout, lists the views that extend it. See [docs/blade-layouts.md](docs/blade-layouts.md).

### Go to Form Request
Palette list of Laravel form request classes and the fields in their `rules()`. See [docs/go-to-form-request.md](docs/go-to-form-request.md).

### Go to Middleware / Job / Event
Palette list of Laravel middleware, jobs, events, listeners, observers, notifications and mailables, tagged by kind (also seeders and factories). See [docs/go-to-app-class.md](docs/go-to-app-class.md).

### Livewire: Find Member Usages
Palette list of where a Livewire view uses its component's methods and properties (`wire:click`, `wire:model`, `$wire.x`), from the class or the view. See [docs/livewire-usages.md](docs/livewire-usages.md).

### Go to Artisan Command
Palette list of your custom Artisan commands (class `$signature` / `AsCommand` and `Artisan::command` closures), jump to the definition. Nothing is run. See [docs/go-to-artisan-command.md](docs/go-to-artisan-command.md).

### Go to Config / Service Provider
Palette list of `config/*.php` files (by their `config('name')` key) and service providers with their `register()`/`boot()` methods. See [docs/go-to-config-provider.md](docs/go-to-config-provider.md).

### Livewire: Go to Property / Action
Palette list of the public properties and methods of the Livewire component open in the editor (from its class or its view). See [docs/livewire-members.md](docs/livewire-members.md).

### Switch Keymap Preset
Palette command that switches between the `compact-65` (no F-keys), VSCode, PhpStorm, Sublime, Notepad++ and Lapce keymaps without opening Settings. See [docs/keymap-presets.md](docs/keymap-presets.md).

### CSS / Tailwind Classes in Use
Palette list of every class name used in `class="..."` attributes under `resources/`, most used first, with counts; jumps to the first use. See [docs/css-classes-in-use.md](docs/css-classes-in-use.md).

### Find View Usages
Palette list of every controller, route, mailable or Blade view that renders or includes the Blade view open in the editor (the reverse of Go to Template). See [docs/view-usages.md](docs/view-usages.md).

### Go to Inertia page
Go to Definition on the page name in `Inertia::render('Users/Index')`, `inertia(...)` or `Route::inertia(...)` opens the Vue/React/Svelte page under `resources/js/Pages`. See [docs/go-to-inertia.md](docs/go-to-inertia.md).

### Go to WordPress template / include
Go to Definition on `get_template_part()`, `get_header()`, `locate_template()` and `require __DIR__ . '/x.php'` strings opens the file. See [docs/go-to-wp-template.md](docs/go-to-wp-template.md).

### Go to Asset
Go to Definition on `asset('css/app.css')`, `mix()` and `@vite([...])` opens the file under `public/`, `assets/` or the project root. See [docs/go-to-asset.md](docs/go-to-asset.md).
### Livewire: Go to Handler
Go to Definition on `wire:click="save"`, `wire:model="email"` or `$wire.save()` in a Livewire view opens the component class at that method or property. See [docs/livewire-handlers.md](docs/livewire-handlers.md).

### Go to Model
Palette list of Eloquent models and their relationships (`hasMany`, `belongsTo`, ...). See [docs/go-to-model.md](docs/go-to-model.md).

### Go to Env / Config Key
Palette list of `.env` variable names and Laravel `config/*.php` keys. See [docs/go-to-env-config-key.md](docs/go-to-env-config-key.md).

### Go to Translation
Palette list of Laravel translation keys from PHP group files (dot-flattened, e.g. `auth.failed`) and JSON lang files, jumping to the defining line. See [docs/go-to-translation.md](docs/go-to-translation.md).

### Go to Blade Component
Palette list of every `<x-...>` and `<livewire:...>` component in the project. See [docs/go-to-blade-component.md](docs/go-to-blade-component.md).

### Go to Route
Laravel and Symfony route list in the command palette ("Go to Route") jumping to controller methods. See [docs/go-to-route.md](docs/go-to-route.md).

### Emmet
Expand HTML (`ul>li.item*3`) and CSS (`m10-20`) abbreviations with `Ctrl+Alt+M`. See [docs/emmet.md](docs/emmet.md).

### Completion popup
LSP results, imported snippets, language keywords, project strings (route, config, env, view and translation names) and buffer words in one fuzzy-ranked popup. See [docs/completion-popup.md](docs/completion-popup.md).

### Themes and icons marketplace
Install colour themes and icon themes from the registry in the Themes & Icons panel. Theme/icon packages are validated data packages (HTTPS-only, size-bounded, no executable files); executable WASI plugins install through the same hardened pipeline. See [docs/data-only-marketplace.md](docs/data-only-marketplace.md).

### VS Code extension import
`lapce --install-vsix FILE...` imports the declarative parts of `.vsix` files: colour themes, icon themes, snippets, language configuration and TextMate grammars. Extension code (`main`) is never executed; such extensions are marked "contains code, not loaded". Packages from [Open VSX](https://open-vsx.org) work; check each extension's licence. See [docs/vsix-import.md](docs/vsix-import.md).

### VS Code API shim (experimental)
An opt-in, deliberately small CommonJS command shim runs in a seccomp-restricted worker process (Linux x86_64 only). It is reached from the command palette (`Experimental VS Code: Run sandbox hello`) and never activates installed extensions. See [docs/vscode-sandbox-experiment.md](docs/vscode-sandbox-experiment.md).

### Keymap presets
`Settings > Core > Keymap Preset`: `compact-65` (default, no Fn layer needed), `vscode`, `phpstorm`, `sublime`, `notepad-plus-plus` and `lapce` (original defaults). Your `keymaps.toml` always wins. See [docs/keymap-presets.md](docs/keymap-presets.md).

### AI Chat
Chat/Ask/Plan panel against any OpenAI-compatible server (Ollama by default), with explicit project-read approval, slash commands (`/plan`, `/ask`, `/btw`, `/learn`), user-curated project knowledge notes folded into chat prompts, and inline AI edits (`Ctrl+Shift+I`). See [docs/ai-chat.md](docs/ai-chat.md) and [docs/inline-ai-edit.md](docs/inline-ai-edit.md).

### External agents over ACP
Run an external agent (opencode, or a Claude Code/Codex adapter) from the chat panel. Sessions are remembered per project, and the Agents list is a cross-workspace view over recent projects: browse, resume (foreign sessions open their project as a tab) and delete. See [docs/agent-acp.md](docs/agent-acp.md), [docs/agent-acp-panel.md](docs/agent-acp-panel.md) and [docs/agent-sessions.md](docs/agent-sessions.md).

### MCP servers (probe)
Workspace `.lapce/mcp.json` servers are handshaken and tool-listed on demand into a status buffer; tool calls stay gated until the review UI owns them. See [docs/mcp.md](docs/mcp.md).

### Scheduled prompts
Recurring and daily prompts fire into the chat panel from `.lapce/scheduled-prompts.json`, managed from the panel. See [docs/scheduled-prompts.md](docs/scheduled-prompts.md).

### Walkthroughs
Export a chat run as a Markdown walkthrough, and browse past exports from the palette. See [docs/walkthroughs.md](docs/walkthroughs.md).

### Git worktrees and push
Palette commands create a worktree on a timestamped branch in a sibling directory and open it as a tab (its own chat sessions and schedules), list and open worktrees, remove them behind a confirm dialog, and pull (fast-forward only) or push the current branch with toast feedback. See [docs/worktrees.md](docs/worktrees.md), [docs/git-pull.md](docs/git-pull.md) and [docs/git-push.md](docs/git-push.md).

### AI commit messages
Generate a commit message from the staged diff in the source control panel. See [docs/ai-commit-messages.md](docs/ai-commit-messages.md).

## Install

Linux x86_64 binaries are attached to [GitHub releases](https://github.com/izdrail/editor.izdrail.com/releases) when a version tag (`vX.Y.Z`) is pushed. To build from source, see [docs/building-from-source.md](docs/building-from-source.md).

## CI

* **Linux build** runs on every branch push on GitHub-hosted `ubuntu-22.04` and publishes the binary as a workflow artifact.
* **Linux release** runs only for `vX.Y.Z` tags and attaches the Linux binary to the release.
* **CI** (build and doc tests) runs on pull requests and manual dispatch.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Release steps are in [docs/new-release.md](docs/new-release.md).

## License

This project is a derivative of Lapce, copyright the Lapce contributors, and is released under the Apache License 2.0, the same license as upstream. Upstream copyright and attribution notices are retained. See [LICENSE](LICENSE).
