# Wrap Selection in Tag

Command palette: **Wrap Selection in Tag** (`wrap_selection_in_tag`, no default key). Pick an element from the list and the selection becomes `<tag>selection</tag>`. With no selection the word at the caret is wrapped; with several selections each one is wrapped. It is one undo step.

- A single line is wrapped inline: `hello` becomes `<b>hello</b>`.
- A multi-line selection puts the tags on their own lines, the closing tag at the indent of the first selected line.

The list: div, span, p, a, section, article, header, footer, main, nav, aside, ul, ol, li, table, tr, td, th, form, label, button, strong, em, code, pre, blockquote, h1-h3, template, and the Blade/Livewire wrappers `x-slot`, `x-card`, `x-alert`, `livewire:component`. Type any name when nothing in the list matches and press Enter; attributes are allowed (`div class="card"`) and only the name goes on the closing tag. Typed text may not contain `<`, `>` or a line break and is limited to 200 characters. Selections over 1 MB are skipped.
