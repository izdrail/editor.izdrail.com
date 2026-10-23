# x-data property completion in Alpine expressions (item 365)

Inside an Alpine expression attribute value (`x-show="op`, `@click="to`,
`:class="op`, `x-text`, `x-html`, `x-if`, `x-init`, `x-effect`,
`x-model`, `x-modelable`, `x-for`, `x-id`, `x-intersect`, `x-trap`,
`x-on:*`, `x-bind:*`), the popup now offers the properties of the
nearest enclosing `x-data` object, from the same tag or an ancestor.

How it works:

- A line check confirms the cursor sits in the quoted value of an
  Alpine expression attribute (plain attributes like `class`/`title`
  and Livewire `wire:*` values are excluded).
- A naive tag stack over the last 16k characters before the cursor
  finds the innermost open tag carrying `x-data`: quoted values hide
  `>`, `</name` pops to the matching open tag, void and unclosed tags
  stay on the stack.
- The object literal's top-level keys are offered: plain and quoted
  keys, methods (`toggle() { ... }`), shorthand idents; spread entries
  are skipped; a truncated object (still being typed) yields the keys
  completed so far. Non-ASCII identifier keys (`caf\u{e9}`, `na\u{ef}ve`)
  are valid JavaScript and are offered too.

Limits: attribute values that started on a previous line are not
recognised, and `x-data="fn()"` component references are not resolved.
Items blend as Project priority (item 358) from the first typed
character (item 359).

Diagnosis: `LAPCE_ALPINE_COMPLETION_DEBUG=1` logs the line, the buffer
tail and the parsed keys on every invocation (temporary, item 365b -
remove once the host ancestor-scope question is settled).

Tests: `parses_x_data_keys`, `finds_x_data_on_the_innermost_open_tag`,
`offers_x_data_properties_in_expression_values`,
`offers_x_data_properties_from_an_ancestor_tag`,
`offers_unicode_x_data_keys`,
`recovers_x_data_scope_after_malformed_prior_markup`.
