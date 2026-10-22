# Hover on a component tag name (item 349)

Hovering the name of `<x-alert` / `</x-forms.input` now shows the
component's file path (view under `resources/views/components` or class
under `app/View/Components`) and, for anonymous components, the `@props`
declaration line from the saved file. Hovering `<livewire:admin.user-profile`
shows the Livewire class or view path. Attribute hover and prop go-to
definition are unchanged; an unknown component shows nothing.

Tests: `component_tag_hover`.
