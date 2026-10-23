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
stays out of context, as do deeper paths (`$wire.$js.`).

Limits: inline `<script>` bodies and `@script` blocks are not member
contexts; the view must map to a component class under
`app/Livewire/**` or `app/Http/Livewire/**` (Volt/anonymous components
have no class, so no members).

Tests: `detects_livewire_wire_member_context`,
`offers_livewire_wire_members`, `detects_livewire_wire_magic_context`,
`offers_livewire_wire_magics`.
