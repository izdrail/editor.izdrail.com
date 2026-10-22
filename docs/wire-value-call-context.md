# Helper calls inside wire: attribute values (item 354)

`wire:click="$dispatch('count.ch"` now completes Livewire event names
instead of the component's methods: inside an open quoted `wire:` value,
a recognised helper call (`route('...`, `$dispatch('...`, `view('...`,
...) wins over the attribute's own model/action context. Unknown call
words (`confirm('...`) and values without a call keep the attribute
context. The call-argument scan moved into `call_context`, shared by the
top-level context and the wire-value branch; behaviour outside `wire:`
values is unchanged.

The bare quote opens the popup here too: with the attribute scan no
longer shadowing the call context, `wire:click="$dispatch('` resolves to
the Livewire-event context, which the 0346 quote gate already covers.

Tests: `dispatch_call_wins_inside_wire_value`.
