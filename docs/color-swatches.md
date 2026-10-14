# Colour swatches

A small filled square appears before CSS colour literals, drawn in that colour.

Recognised: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`/`rgba()` and `hsl()`/`hsla()` with numeric arguments (comma or space syntax, optional `/ alpha`, percentages).

Files: `.css .scss .sass .less .styl .pcss .html .htm .vue .svelte .twig .js .jsx .ts .tsx .mjs` and `.blade.php`.

Not recognised: named colours (`red`), `var(--x)`, `currentColor`, `hwb()/lab()/oklch()`, and Tailwind class names such as `bg-red-500`.

Turn off with `enable-color-swatches = false` in settings.

## Limits
- Only the first 2000 bytes of a line are scanned, at most 12 swatches per line, and lines over 20,000 bytes are skipped.
- The swatch is phantom text, like an inlay hint: it shifts the text on that line right by two columns and is not part of the file.
- Alpha is applied to the square, so translucent colours look faded against the editor background rather than blended with the real page.
- `#` followed by 3, 4, 6 or 8 hex digits counts, so an issue reference like `#abc` in a comment shows a swatch. `&#8364;` and `a#fff` do not.
