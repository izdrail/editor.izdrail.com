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
