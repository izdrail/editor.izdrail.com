# Livewire layout chain completion (item 380)

A Livewire full-page component ends `render()` with
`return view('livewire.counter')->layout('layouts.app');`. The chain
methods now complete: `->layout('...')` offers the project's Blade
views (dotted names, the `view('...')` list) and `->section('...')`
offers the sections named by `@section` / `@yield` in
`resources/views/layouts`.

The contexts need the `->` receiver; a bare `layout('...')` or
`section('...')` call stays silent (`@section('...')` keeps its Blade
directive meaning). The lists are workspace-wide, so this works in any
PHP file with no component pairing.

Tests: `detects_contexts` layout/section pins,
`offers_livewire_layout_chain`.
