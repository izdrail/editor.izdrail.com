# Project and keyword completions from the first typed character (item 359)

With something typed, the popup blends LSP items with every local source
except buffer words, which still wait for `MIN_WORD_INPUT` (2)
characters. Previously every non-snippet local source waited for two
characters, so a one-character project context (`<div wire:l`) lost its
project items (`loading`) and showed only snippets until a second
character was typed (`wire:lo`). This completes the 0358 ranking, which
already puts project items first once they are present.

Project items exist only when the line is a recognised project context,
so they are relevant from the first character. Keywords now join at one
character too, the usual editor behaviour; buffer words stay gated
because one-character fuzzy matches over the whole buffer are noise.

Tests: `typed_input_keeps_project_items_below_word_minimum`.
