# Rename Tag

Command palette: **Rename Tag** (`rename_tag`, no default key). Put the caret inside the opening or closing tag of an element (the name or the attributes), run the command and pick the new element name from the list (the same list as Wrap Selection in Tag). Both tags are renamed in one undo step. The innermost element around the caret is used.

Parsing is textual and bounded (files up to 1 MB, tags up to 4 KB). Skipped: HTML and `{{-- --}}` comments, `<?php ... ?>`, doctype, void elements (`br`, `img`, ...), self-closing tags, and the bodies of `<script>` and `<style>`. Quotes and `{{ }}` inside attributes may contain `>`. A mismatched closer is tolerated. If no matched element is found a warning toast says so and nothing changes.

You can also type any element name: when nothing in the list matches, Enter uses what you typed (name only; attributes are not used for rename).
