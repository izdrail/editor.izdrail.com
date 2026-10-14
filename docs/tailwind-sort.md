# Sort Tailwind Classes

Command palette: **Sort Tailwind Classes** (`sort_tailwind_classes`, no default key). Sorts the utilities inside static `class="..."` and `className="..."` attributes, in the selection or, with nothing selected, in the whole file. One undo step, plus a toast with the count.

```html
<div class="text-sm p-4 bg-white flex mt-2 rounded md:flex hover:bg-blue-500">
<!-- becomes -->
<div class="mt-2 flex rounded bg-white p-4 text-sm hover:bg-blue-500 md:flex">
```

## How it orders

Groups follow the usual Tailwind output order: container, visibility, position, inset/z-index, margin, display, sizing, flex/transform/cursor, flex and grid layout with gap, overflow, border/rounded/ring, background, fill/object, padding, typography, effects (opacity, shadow, filters), transitions. Variant classes come after plain ones: pseudo-classes (`hover:`, `focus:`), then `dark:`, then breakpoints `sm:` to `2xl:`. Classes the table does not know (your own `btn-primary`) go first, in their original order.

**This is an approximation, not the real Tailwind sorter.** Inside a group the original relative order is kept (the real one orders by CSS property and value), and the table is for Tailwind v3 core utilities. The result can differ from `prettier-plugin-tailwindcss` in fine ordering. Running it twice changes nothing.

## What is never touched

Values with `{{ }}`, `{!! !!}`, `<?php`, `@if`/`@class`, `$`, `{...}`, backticks or a line break; `data-class=`; files over 1 MB. Only attributes quoted with `"` or `'` are read.
