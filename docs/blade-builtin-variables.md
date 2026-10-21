# Blade's built-in variables complete (item 345)

Typing `{{ $` in a Blade view now also offers the variables Blade provides
without any assignment the scanner could see:

- `$errors` - always shared with every view;
- `$loop` - while an `@foreach` / `@forelse` is open (with the detail
  listing `->index`, `->iteration`, `->first`, `->last`, `->remaining`,
  `->count`, `->depth`);
- `$message` - inside an `@error` block;
- `$slot` and `$attributes` - in a component view (one with `@props`).

A view that defines its own variable with one of these names keeps it and
gets no duplicate. Open blocks are counted on the comment-stripped text,
so a `{{-- @foreach --}}` comment cannot fake a `$loop`.

Tests: `blade_variables_adds_builtin_scope`; the exact-list expectations in
`blade_variables_collects_view_scope` and
`blade_variables_excludes_non_definitions` now include the built-ins.
