# Blade hover help

Hovering in a `.blade.php` file shows help from local text, without asking a language server:

- **Directive** (`@foreach`, `@can`, `@vite`, `@props`, ~70 built in): the syntax and a one-line description.
- **Attribute of an `<x-...>` tag**: the component's declaration of that prop (the `@props` entry or constructor parameter, with its default) and its file and line. Multi-line tags work (30 lines back, 16 KB).

If neither applies, hover falls through to the language server as before.

Limits: the directive table follows Laravel 10/11 Blade. Custom directives from `Blade::directive`, package directives, and `@@escaped` or e-mail-like `a@b` text get no help. Prop help uses the same static `@props` scan as go-to (see [blade-props-goto.md](blade-props-goto.md)); `@aware` and dynamic `@props` are not read.

## Livewire views

In a view under `resources/views/livewire/`, hovering a name in a `wire:model="email"` / `wire:click="save"` value, or a `$email` use, shows the matching `public` property or method of the component class (`app/Livewire/...`, or `app/Http/Livewire/...` for v2), with its declaration and file:line. See [livewire-hover.md](livewire-hover.md).
