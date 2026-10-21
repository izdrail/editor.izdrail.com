# `@livewire('...')` completes component names (item 344)

Typing `@livewire('` in a Blade view offers the project's Livewire
components (the same list `<livewire:` tag completion uses, item 337):
`Admin/UserProfile.php` completes as `admin.user-profile`. The replace
range covers the whole dotted name via item 343's `dotted_name_context`,
now extended with `Kind::LivewireName`, so accepting after
`@livewire('admin.us` yields `@livewire('admin.user-profile')`.

Tests: `offers_livewire_directive_names`, plus the new
`dotted_name_context` case.
