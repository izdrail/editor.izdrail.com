# Lexilla fold levels drive code folding (item 446)

Fold ranges now come from the lexer's own fold analysis where it produces
one; indentation stays the language-independent fallback.

## Pipeline

- `lexilla-rs` runs the lexer's `Fold` pass on every snapshot (`lx_lex`
  fold flag, with the `fold` and `fold.html` properties set: the
  PHP-family lexers and LexHTML gate level computation on them) and
  exposes the per-line Scintilla level words as `Snapshot::levels()`.
- `folding::level_ranges` turns levels into `FoldRange`s following the
  container contract: a header-flagged line opens a fold at its own
  (low-word) level; the fold ends at the first later non-blank line whose
  level is at or below the header's, so each block's closing brace or tag
  stays visible. Trailing blank lines stay visible at EOF.
- Two level word formats are honoured: plain Scintilla (number and flags
  in the low bits, e.g. LexHTML closing tags carry the inner level) and
  the PHP-family packing `next<<16|flags|current`, where a closing-brace
  line reports its pre-decrement level low and the post-decrement level
  high; the high half is the fold boundary for lines inside the fold.
- `Syntax::parse` uses level ranges when they are non-empty and falls
  back to `indentation_ranges` otherwise (plaintext, lexers without a
  Fold implementation such as Twig).

## What this fixes

Indentation folding misses brace-delimited blocks that do not indent:
zero-indented PHP (generated or compact code), K&R closing styles, and
HTML tag nesting in flat markup. The lexer sees braces and tags, not
columns, so those fold now. Well-indented code gets the same ranges as
before (verified by the pre-existing fold tests, which pass through the
level path unchanged).

## Tests

- `folding::tests::level_ranges_follow_scintilla_and_packed_php_levels`
  - packed PHP words, plain HTML words, EOF trailing blanks, flat input.
- `lexilla::tests::lexilla_fold_levels_drive_ranges_for_zero_indented_php`
  - discriminating case: zero indentation means indentation ranges are
    empty, so a pass proves the level path.
- `lexilla::tests::html_tag_levels_drive_folding` - tag nesting folds.
