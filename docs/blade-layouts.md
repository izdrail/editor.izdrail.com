# Go to Blade Layout / Section

Command palette: **Go to Blade Layout / Section** (`palette.blade_layouts`).

- In a view with `@extends('layouts.app')`: lists the layout file and each `@yield` / `@stack` slot in it. Pick one to jump to that line.
- In a layout (a view that has `@yield` or `@stack`): lists every view under `resources/views` that `@extends` it, at the `@extends` line.
- Anywhere else the list is empty.

Static text scan only; no PHP is run. Limits: depth 8, 5000 files, 1 MB per file, 500 results, symlinks not followed. Package views (`pkg::name`) and names containing `..` or slashes are ignored. Only the first matching directive on a line counts. Only the first `@extends` in a file is used.

Status: unit tests cover parsing, both directions, empty cases and random-input fuzz. The palette GUI is not yet verified.
