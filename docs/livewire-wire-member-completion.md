# Livewire $wire member completion (item 368)

Typing `$wire.` inside an Alpine expression attribute value
(`@click="$wire.sa`, `x-show="$wire.cou`, `x-on:*`, `x-bind:*`,
`:class`, ...) or a `wire:` attribute value offers the paired Livewire
component's members as Project items: public properties first, then
public methods, from the same static class scan as `wire:model` /
`wire:click` completion. A name that is both a property and a method
appears once, as the property.

The context needs the caret inside an open quoted value of one of those
attributes, ending with `$wire.` plus an optional partial member name
(letters, digits, `_`). `class=` and `x-data=` values are not member
contexts. Precedence: `$wi` (no dot yet) stays the magic-name context
(item 367); `$wire.dispatch('...` keeps the Livewire event context;
`$wire.$` is a member-of-`$wire` position and no longer raises the
Alpine magic list (the magic context now also rejects a `.` before the
`$`).

`$wire.$` magics (item 371): `$wire.$` (or a partial) offers the
object's own magics - `$set`, `$get`, `$toggle`, `$refresh` as methods,
`$parent` and `$js` as properties - aligned with the `$wire` hover
(item 350). The partial must start a known magic name, so `$wire.$foo`
stays out of context. Deeper paths are handled by the more specific
contexts: `$wire.$js.` is item 372 below, any other dotted path is out
of context.

`$wire.$js.<name>` (item 372): inside an Alpine expression attribute
(`@click`, `x-on:`, `x-bind:`, `x-show`, ...) or a `wire:` attribute,
`$wire.$js.` offers the `$js` functions registered in the CURRENT view
- the `$js('name', fn)` registrations in its `@script` blocks (Livewire
3). Names come from a textual scan of the saved file (same read model
as the x-ref list), so a registration appears after the file is saved.
`$wire.$j` / `$wire.$js` without the second dot stay the `$wire` magic
context; a lookalike (`$wire.$jsx.`) or a deeper path
(`$wire.$js.copy()`) is out of context; plain attributes (`class=`)
and `x-data` are not contexts.

Limits (v1): the scan is textual and covers only the current file, so
names registered in a shared `@script` elsewhere or via a build step
are not offered; only single-segment names are matched.

`$entangle('...')` (item 373): the first quoted argument of
`$entangle(`, `$wire.entangle(` (and a bare `entangle(`, matching the
call-helper convention) offers the paired component's public
properties - the same scan as `$wire.<member>`, minus the methods,
which are not valid entangle arguments. Detection is the generic
open-call-argument helper, so it works inside `x-data` object values,
`@click`/`wire:` attribute values and `@script` blocks alike; once the
argument string closes (`$entangle('show', `) the context ends.

`$wire.$watch('...')` (item 374): the first quoted argument offers the
paired component's public properties (only properties can be watched).
A bare Alpine `$watch('...')` stays out of context: it watches x-data
keys, which this line-only context cannot see.

Tests: `detects_livewire_wire_member_context`,
`offers_livewire_wire_members`, `detects_livewire_wire_magic_context`,
`offers_livewire_wire_magics`, `detects_livewire_js_context`,
`offers_livewire_js_names`, `detects_livewire_entangle_context`,
`offers_livewire_entangle_props`, `detects_livewire_watch_context`,
`offers_livewire_watch_props`.
