# Go to template / component (Blade and Twig)

"Go to Definition" (F12 / Ctrl+click as bound) opens the template or component under the
cursor directly, before any language server is asked. It works without a language server.

Blade (`.blade.php`, and `view(...)` calls in any `.php` file), relative to `resources/views`:

- `@include`, `@includeIf`, `@includeWhen($cond, 'x')`, `@includeUnless`, `@includeFirst`,
  `@extends`, `@each`, `@component`, `view('a.b')`, `View::make`, `Route::view`, `markdown`:
  `a.b` opens `a/b.blade.php` (or `a/b.php`, `a/b/index.blade.php`).
- `<x-alert>`, `<x-forms.input>`: opens `components/alert.blade.php`,
  `components/forms/input.blade.php` (or `.../input/index.blade.php`), else the class
  `app/View/Components/Forms/Input.php` (kebab-case becomes StudlyCase).
- `<livewire:admin.user-profile>` and `@livewire('admin.user-profile')`: opens
  `app/Livewire/Admin/UserProfile.php` (Livewire 3), else `app/Http/Livewire/...` (Livewire 2), else
  `resources/views/livewire/admin/user-profile.blade.php`.

Twig (`.twig`), relative to `templates/`:

- `{% extends %}`, `include`, `embed`, `import`, `from`, `use`, `{{ include('x') }}`, `source('x')`:
  opens `templates/<name>`.
- `<twig:Alert>`, `<twig:Form:Input>`, `{{ component('Alert') }}`: opens
  `templates/components/Alert.html.twig` (`Form:Input` is `Form/Input.html.twig`).

Rules: only the first (template name) argument counts, the file must exist, and names with `..`, a
leading `/` or `::` are ignored. `@Bundle/...` Twig namespaces and Blade `vendor::view` namespaces are
not resolved, and Livewire components named by class (`Livewire::component`) are not resolved. If nothing matches, normal Go to Definition runs.
Nothing is executed; the line under the cursor is read and file existence checked inside the workspace.
