# Completion and keywords

## Request and select a suggestion

In Insert mode, put the caret after an identifier or receiver and press Ctrl+Space. Type more characters to filter, use Up/Down or PageUp/PageDown to move through the list, and Enter to insert the selected result. Escape closes the list. A suggestion's first line is its insertion label; the second line shows owner/source/signature metadata. Hover that second line to read long metadata in a wrapped tooltip.

An empty list can mean no result matches the current prefix, file type or project, not that the shortcut failed. Check the active preset in Keyboard Shortcuts if Ctrl+Space is intercepted by your OS or input method.

## Laravel static facade methods

Open a Laravel project folder, then a PHP file. For example:

```php
<?php
use Illuminate\Support\Facades\Route;

Route::
```

Press Ctrl+Space after `::`. Type `ge` to filter, select `get` and press Enter. The fallback inserts only `get`, not arguments or imports. Imported aliases and absolute facade paths work too:

```php
use Illuminate\Support\Facades\Route as Router;
Router::
\Illuminate\Support\Facades\Route::
```

Bundled Laravel 12 method packs cover Route, Auth, Cache, DB, Schema, Storage, Log, Request, Validator, Response, App, Config, View, Session, Redirect and URL. These are explicitly labelled **Laravel 12 baselines**, not installed-version or runtime-type inference. A language server can supply higher-priority results for the actual project.

Strings, comments, unrelated class imports and shadowed aliases do not bind to these packs. Inside a namespace, import the facade or use its absolute name. The resolver only reads code before the caret, so later declarations need a language server. See [Laravel completion](../laravel-completion.md) for scope and source manifests. Eloquent runtime member inference is not provided by these static packs.

## Snippet keywords

A snippet is a code template offered in completion. Type its prefix, choose its snippet result and press Enter. Tab moves to the next editable placeholder; Shift+Tab moves back. Examples:

| File / project | Prefix | Template |
| --- | --- | --- |
| PHP in Laravel | `route-get` | Route to a controller method |
| PHP in Laravel | `lv-model` | Model class |
| PHP in Laravel | `lw-component` | Livewire component |
| Blade template | `foreach` | Blade loop |
| WordPress PHP | `wp-action` | Action hook |
| Symfony PHP | `sf-controller` | Controller |
| Twig template | `for` | Twig loop |
| JavaScript / TypeScript | `fn` | Function |

PHP framework snippets depend on static project detection. Blade files use the Blade pack rather than the PHP/Laravel pack. In Blade, a typed `@` or `<` before a supported prefix is replaced with the template rather than duplicated. See the [full snippet prefix list](snippet-reference.md) and [pack notes](../snippet-packs.md). These prefixes are editor expansion keywords, not extra syntax accepted by the language.

## AI ghost text is separate

Ghost text requires its configured AI backend and does not use the static Laravel method packs. With an inline suggestion visible in Insert mode, Tab accepts it, Escape dismisses it and Alt+LeftBracket / Alt+RightBracket cycle suggestions. Alt+Backslash requests an inline suggestion when none is visible. Popup/search/modal focus can take precedence over these keys.

Ctrl+Shift+I opens Inline AI Edit for the selected code, or the current line if there is no selection. Enter submits the instruction; a successful reply replaces the captured range. Escape cancels. A failed or stale reply must not replace a newer edit. This is different from merely inserting a completion label. See [Inline AI Edit](../inline-ai-edit.md) before sending private code to a backend. This guide does not configure or contact any AI service.
