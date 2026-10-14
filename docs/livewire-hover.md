# Livewire hover

In `resources/views/livewire/admin/user-profile.blade.php`, hover:

- the name in `wire:model="email"`, `wire:model.live="email"`, `wire:click="save"` or `wire:click="save(1)"`;
- a `$email` use such as `{{ $email }}`.

The popup shows the component's declaration (`public string $email = '';` or `public function save(): void`) and its file and line. The class is `app/Livewire/Admin/UserProfile.php`, or `app/Http/Livewire/...` for Livewire v2.

Limits: only `public` members. The attribute value must be a bare name (`form.name` hovers `form`, expressions like `!$open` or `a->b` give no help). Properties set in `mount()` without a declaration, `#[Computed]` methods used as `$this->x`, traits and parent classes are not searched. A view without a matching class gets no help. Static text scan, nothing is run, 1 MB per class file.
