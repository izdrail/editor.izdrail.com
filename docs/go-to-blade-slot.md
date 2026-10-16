# Go to Blade slot (@stack / @push, @yield / @section)

In a `.blade.php` file, put the cursor on the name and run Go to Definition:

- `@stack('scripts')` opens the first `@push('scripts')` (or `@prepend`, `@pushOnce`, `@pushIf`).
- `@push('scripts')` opens the `@stack('scripts')` in the layout.
- `@yield('content')` opens `@section('content'`, and `@section('content')` opens `@yield('content')`.

Files are searched under `resources/views` in a stable order and the first match wins. For every user of a
slot use "Blade Layout Map". Limits: depth 8, 5000 files, 1 MB per file, symlinks skipped. Nothing is
executed. If nothing is found, normal Go to Definition runs.
