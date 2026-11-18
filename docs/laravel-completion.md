# Laravel Route completion

In a detected Laravel PHP project, Ctrl+Space after `Route::` lists Route facade
methods even when no PHP language server responds. Imported aliases such as
`use Illuminate\Support\Facades\Route as Router; Router::` and the full
qualified facade name are supported. Type part of a method to filter, then
Enter to insert only the method name. No arguments or imports are guessed.

The popup keeps the method label on its own line and the full owner path plus
signature below it. Kind badges and existing source-priority ranking remain.
Local Route methods use the project tier, not generic buffer words. Metadata
class/function qualified names and language-server descriptions also appear
below their insertion labels. Keyboard page movement and popup placement use
the new two-line row height.

This is an explicitly labelled Laravel 12 offline baseline, generated from the
Route facade declarations. It is not an inferred receiver type or a promise
that every method exists in another Laravel version. Sources and digest are
recorded in metadata/laravel/manifest.json. Installed language-server results
remain higher priority.

The fallback refuses non-Laravel projects, strings, comments, unknown receivers,
shadowed Route imports/classes and unimported namespaced Route names. Imports
inside comments or strings cannot bind a facade alias. Grouped imports, multiple class imports and namespace aliases are supported. Scanning is bounded to 512 KiB; larger documents rely on LSP.

This slice covers Route static methods. Eloquent model/member inference and
other facade method packs remain separate work. Existing quoted route-name,
view, config and middleware completions are unchanged.

## Additional facade packs

Auth, Cache and DB now use the same guarded static-member fallback as Route.
Each pack contains only that facade's declared methods. Full facade names and
imported aliases work; unrelated imports and user-defined shadow classes do
not bind to a bundled facade. These are also Laravel 12 baselines, not a runtime
or installed-version check. Their source digests and counts are recorded in
metadata/laravel/facade-manifest.json. Auth, Cache and DB have 58, 60 and 107
methods respectively. Other facades and inferred Eloquent members are not part
of these packs.

## Import scope hardening

Facade imports are resolved only in the active PHP namespace block. Aliases
from a previous namespace block cannot leak into another one. Trait insertion
and closure/function-local use syntax do not create namespace imports.
Class imports stay separate from function and constant imports. Fully qualified
facade names ignore aliases; class shadows and duplicate aliases fail closed.
Mixed HTML text, strings and comments cannot forge PHP imports.

Import semantics reference: https://www.php.net/manual/en/language.namespaces.importing.php
This is a bounded completion resolver, not a PHP validator. It only reads code
before the cursor, so declarations after the cursor still require an LSP.

## Full signature hover

Hover over a suggestion's metadata line to read the full owner path and method
signature in a wrapped tooltip. The compact list still uses two rows and an
ellipsis for long metadata, so scrolling stays predictable. LSP-provided
label-detail signatures are retained alongside the owner and detail, with
identical fields shown only once. Tooltip content is plain text, control
characters are replaced, and very large metadata is capped at 4,096 characters
with an explicit ellipsis. Selecting a suggestion still inserts only its label.
