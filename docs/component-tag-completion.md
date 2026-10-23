# Component tag completion context (item 340)

Inside a Blade component tag name (`<x-forms.in`) or a Livewire tag name
(`<livewire:admin.us`):

- The completion list offers ONLY the project's component names: no
  snippets, keywords or buffer words. Accepting the top row can no longer
  insert a directive snippet into the tag.
- Items appear immediately after `<x-` / `<livewire:`, before any letter
  is typed. `ScoredCompletionItem.show_on_empty` marks the items allowed
  with an empty input, and `update_completion` now filters local items at
  once instead of waiting for an LSP response that never comes for a plain
  Blade file.
- The replace range covers the whole dotted name. `completion_word_start`
  extends the word start backwards over `.`- and `-`-joined segments, and
  `builtin_snippets::tag_name_word_len` stops it right after the `<x-` /
  `<livewire:` marker. Every consumer uses it: the filter input and
  `completion.offset` in `update_completion`, and both branches of
  `apply_completion_item` (text_edit origin and plain selection), so the
  snippet origin from item 335 stays consistent too. Accepting
  `admin.user-profile` after `<livewire:admin.us` yields
  `<livewire:admin.user-profile`, not `<livewire:admin.admin.user-profile`.

Dotted names in quoted calls (`route('admin.us')`) still replace only the
last segment; that is the same bug class but outside the tag context and
left for a follow-up.

Tests: `tag_name_word_len_covers_dotted_tag_names`,
`empty_input_offers_only_opted_in_locals`.

## `<x-` tag vs Alpine attribute (item 342)

Item 341 combined the word-start extensions with `max(hyphen, tag)`, which
was wrong for `<x-`: `hyphen_word_len` pulled the `x-` marker into the
word, so the fuzzy input became `x-al` (component labels like `alert` never
match, Alpine `x-*` snippets do) and `start_offset` stayed pinned to
`completion.offset`, keeping the popup on the stale update path with the
local items computed before the marker existed. Accepting a snippet there
also replaced `<x-al` through the `replaces_leading_char('<')` range,
destroying the tag.

`tag_name_context(before)` now decides which extension applies: in a tag
name (right after `<x-` / `<livewire:` or inside the dotted name) only
`tag_name_word_len` extends, so the word never includes the marker and the
offset moves as the name grows, forcing a fresh item fetch in tag context
(components only, via item 341). After whitespace (`<div x-d`,
`<x-alert x-d`) the hyphen extension runs as before and Alpine `x-*`
snippets still complete. The empty-input gate also opens on `<x-` even
though `-` is not a trigger char.

Tests: `tag_name_context_distinguishes_tag_from_attribute`.

Component prop completion (item 375): inside a component tag, after the
tag name, attribute-name positions offer the component's declared props
- `@props([...])` keys and list items of a view component, constructor
parameters of a class component, in declaration order, camelCase keys
offered as kebab-case (`maxLength` -> `max-length`, matching the go-to
mapping). The caret must be outside quotes and not at a value position
(`foo=`, `foo="..."`, the self-closing `/` or right after a closing quote
before the separating space); the tag-name position
keeps the item 340 tag-name context. Framework attribute positions keep
their existing completions: `@cl` still offers Alpine events and
`wire:cl` wire: attribute names (both checked earlier, so props defer);
only `x-*` positions offer nothing of their own, while the `:disabled`
binding prefix completes props. Accepting a kebab
prop replaces the whole hyphenated partial via the existing Blade
hyphen word range.

Limits (v1): the tag must open on the same line (multi-line tags are
not seen); `<x-slot>` takes no props; unknown components offer nothing;
the merge with snippets/words follows the normal value-position rules.

Tests: `detects_blade_prop_context`, `offers_blade_component_props`
(project_completion), `prop_names_keys_and_list_items`,
`ctor_params_in_order` (blade_props).
