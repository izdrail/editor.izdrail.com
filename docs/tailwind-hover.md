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

Arbitrary values: `w-[13px]`, `bg-[#1da1f2]`, `-mt-[0.5rem]`, `px-[calc(100%_-_1rem)]`. The value
between the brackets is shown verbatim (underscores become spaces, except inside `url(...)`).
Covered prefixes: the spacing/sizing families above, the colour prefixes above (a hex colour with a
`/50` opacity modifier is decomposed to `rgb(...)`), `bg-[url(...)]` as `background-image`, `rounded`
with every side and corner, `z`, `opacity`, `shadow`, `grid-cols`/`grid-rows`, `leading` and
`tracking`. For `text-[...]` and `border-[...]`, which can set either a length or a colour, the
`length:`/`color:` type hint wins (`text-[length:14px]`); without a hint the value shape decides
(`text-[14px]` -> `font-size`, `text-[#333]` -> `color`), and an ambiguous `var(...)` gets no hover.

Not covered: gradient and ring colours, custom values from `tailwind.config.js`, plugins, and
Tailwind v4 differences. A class that matches nothing gets no hover. Only the hovered line is read
(4096 bytes at most, arbitrary values at most 60 bytes); classes built with `{{ }}`, `$` or `@` are
skipped. Nothing is executed. Blade directive and component prop hover still win in Blade files.
