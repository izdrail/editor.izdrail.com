# Rename Tag

Command palette: **Rename Tag** (`rename_tag`, no default key). Put the caret inside the opening or closing tag of an element (the name or the attributes), run the command and pick the new element name from the list (the same list as Wrap Selection in Tag). Both tags are renamed in one undo step. The innermost element around the caret is used.

Parsing is textual and bounded (files up to 1 MB, tags up to 4 KB). Skipped: HTML and `{{-- --}}` comments, `<?php ... ?>`, doctype, void elements (`br`, `img`, ...), self-closing tags, and the bodies of `<script>` and `<style>`. Quotes and `{{ }}` inside attributes may contain `>`. A mismatched closer is tolerated. If no matched element is found a warning toast says so and nothing changes.

You can also type any element name: when nothing in the list matches, Enter uses what you typed (name only; attributes are not used for rename).

Raw-text closing names must be complete: `</scripture>` and `</styles>` inside JavaScript or CSS do not end the raw-text body or expose fake HTML elements to Rename Tag. Real elements after the matching `</script>` or `</style>` remain editable.

PHP boundaries come from the shared PHP lexer, not a substring search for `?>`. Tag-like text inside PHP strings, block comments, backticks, heredocs and nowdocs cannot be renamed. A real `?>` (including one in a PHP line comment) restores HTML parsing. One lazy lex pass supplies all close-tag offsets, even in files with many PHP regions; a failed lex hides the rest conservatively. This does not execute PHP or shell commands.

The shared scanners accept mixed-case long PHP tags such as `<?PHP`, matching PHP tokenization. Non-PHP processing instructions such as `<?xml` always advance the standalone scanner rather than looping without consuming input. Rename Tag keeps its existing processing-instruction skip and 1 MB file limit.

PHP opener checks use the same long-tag and short-prefix whitespace boundaries as highlighting. Processing-instruction lookalikes (`<?phpx`, `<?php/**/`, vertical-tab/formfeed after `<?`) do not hide real HTML after their `?>`. Real PHP regions still shield quoted tag-like text.

Saving a file whose line endings already match no longer adds an empty undo step. Rename Tag followed by Save can be undone with one Undo. Actual line-ending conversions still create an edit.
