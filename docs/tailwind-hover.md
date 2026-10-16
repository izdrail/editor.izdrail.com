# Tailwind class hover

Hover a class inside `class="..."` or `className="..."` in a Blade, HTML, Vue, Svelte, JSX, TSX or
Twig file to see the CSS it produces, plus the variants that apply:

    md:hover:p-4   ->   padding: 1rem;      Applies with: @media (min-width: 768px), :hover

Covered (Tailwind v3 default theme, values checked against tailwindcss v3.4.17
`stubs/config.full.js`): padding, margin (including negative and `auto`), gap, width, height, size,
inset/top/right/bottom/left with spacing, `full`, fractions and `screen`/`min`/`max`/`fit`,
rounded, shadow, text sizes, font weights, display, position, flex, alignment, grid columns and
spans, z-index, opacity, border width, and a few text and overflow helpers.

Colours: the default v3 palette (22 colours, shades 50 to 950, values from tailwindcss v3.4.17
`src/public/colors.js`) for `bg-`, `text-`, `border-` (and `-t -r -b -l -x -y`), `fill-`, `stroke-`, `accent-`,
`caret-`, `decoration-` and `outline-`, with `/50` style opacity. The hover shows `rgb(r g b)` and the hex
in a comment; Tailwind itself emits the same colour through an opacity variable.

Not covered: gradient and ring colours, arbitrary values (`w-[13px]`), custom values from `tailwind.config.js`,
plugins, and Tailwind v4 differences. A class that is not in the table gets no hover. Only the hovered
line is read (4096 bytes at most); classes built with `{{ }}`, `$` or `@` are skipped. Nothing is
executed. Blade directive and component prop hover still win in Blade files.
