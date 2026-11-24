# Laravel-family snippet audit

24 November2026. Existing inventory: Laravel41, Livewire17, Blade108. Complete pre-change prefix/description/template inventory is retained in snippet-audit-before.json. Other framework and JavaScript packs are outside this task.

Naming findings:10 Laravel prefixes lacked lv-, all Livewire/Blade prefixes lacked it. Contextually shared short prefixes overlapped with Twig. Normalize Laravel route prefixes, Livewire to lv-livewire-*, Blade to lv-blade-*. This is an intentional breaking prefix rename requested by the owner; old aliases are not retained because every prefix must start lv-. Templates preserved at this first stage.

Quality/coverage: existing Rust parser tests validate template syntax and key PHP dollar/backslash behavior, but do not establish Laravel runtime compatibility. A-J additions and import/return-type/template review remain open. Do not present normalization as the full expanded library.
