# Blade outline fallback (item 326)

The Document Symbol panel was blank for Blade views without an LSP answer. `blade_outline::outline` now builds a local outline from the buffer: `@section` (with its closing directive as range end; `@section('a', 'b')` is a one-line inline section), `@push`/`@prepend`, `@component`, `@livewire` and bare `@php ... @endphp` blocks (first code line shown as detail). `{{-- --}}` comments, inline `@php(...)` and look-alikes such as `@sectionMissing` are skipped. Used only for `*.blade.php` files and only when the LSP returned no nested symbols (or an error). Capped at 500 symbols and 2 MB of text. Flat list, no nesting.

Limits: static line scan, so a directive that shares a line with other text is not listed, and a quoted comma inside a section name is read as an inline section.

Capture: open a Blade view with sections, an @php block and a @livewire call, open the Document Symbol tab, click an entry and check it jumps to that line.
