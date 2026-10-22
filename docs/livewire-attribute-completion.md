# Livewire attribute name completion (item 356)

Inside a tag, typing `wire:` (or a partial name like `wire:lo`) offers
the Livewire attribute names: the event/action list (`click`, `submit`,
`model`, `poll`, ...) plus the documented attributes (`loading`,
`target`, `navigate`, `key`, `ignore`, `stream`, `transition`, `dirty`,
`offline`, `cloak`, `confirm`, `init`), labelled "Livewire attribute".
The two source lists (`livewire_handlers::EVENTS` and the hover
attribute table, now `pub(crate)`) are deduped. Modifier (`wire:model.`)
and value (`wire:model="..."`) contexts are unchanged, and the tag guard
keeps `wire:` outside elements inert.

Tests: `detects_livewire_attribute_context`, `offers_livewire_attributes`.
