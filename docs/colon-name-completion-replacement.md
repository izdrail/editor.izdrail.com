# Colon-name completion replacement (item 410)

Bug: with `Artisan::call('reports:r')` typed, accepting the fuzzy
first-ranked item inserted a hybrid - the replace range covered only
the segment after the colon, so `reports:` stayed and the accepted
label (`reload`) was grafted on, producing the nonexistent
`reports:reload`.

`completion_word_start` already extends the completion word start
backwards over `.`-joined segments in dotted-name contexts; it now
also extends over `:`- and `-`-joined segments in the artisan
command context (`colon_name_context` / `colon_word_len` in
`project_completion.rs`). The replace range covers the whole typed
name from the opening quote, and because the filter input uses the
same word start, the popup now filters on the full typed prefix
(`reports:r` matches `reports:send`, not unrelated one-word names).

The extension is string-context gated, so `::` in ordinary PHP code
is never treated as a word join, and non-PHP files keep the plain
code boundary.

Tests: `colon_name_context_and_word_len`,
`completion_word_start_tests::colon_context_covers_the_whole_typed_name`
and its boundary/dotted/non-PHP pins.
