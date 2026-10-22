# Livewire event name completion (item 353)

Inside `$dispatch('...')`, `$this->dispatch('...')`,
`$wire.dispatch('...')`, `Livewire.dispatch('...')` and the Livewire 2
`->emit('...')` / `->emitTo(...)` / `->dispatchBrowserEvent('...')` calls,
completion offers event names gathered from the project's dispatch sites
and listeners: the first quoted argument of those calls plus
`#[On('name')]` attributes, scanned in `app/` and `resources/views/`
(labelled "Livewire event"). Dotted names (`count.changed`) complete as
one word. A longer identifier tail (`mydispatch(`) is not a context.

Textual scan, nothing executed. Bounded: depth 8, 400 files, 1 MB per
file, 300 names, symlinks skipped; runs only in this context.

Tests: `detects_livewire_event_context`, `offers_livewire_events`.
