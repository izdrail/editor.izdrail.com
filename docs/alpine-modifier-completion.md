# Alpine event modifier completion (item 348)

Typing a dot after an Alpine event attribute name offers the Alpine v3
modifiers, for both `x-on:click.` and the `@click.` shorthand, before any
`=`:

- every event: prevent, stop, once, self, capture, passive, window,
  document, debounce.250ms, throttle.250ms
- mouse events (click, dblclick, mousedown, mouseup): + outside
- keydown / keyup: + enter, escape, space, tab, backspace, the arrow keys,
  shift, ctrl, alt, meta

Blade directives (`@foreach`) are not events and open nothing; an
attribute value (`@click="open = true`) keeps the existing behaviour. The
list opens on the dot and filters per keystroke; accepting replaces only
the partial modifier.

Tests: `offers_alpine_modifiers`.
