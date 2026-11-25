<?php
return Illuminate\Foundation\Application::configure(basePath: dirname(__DIR__))
    ->withProviders([Livewire\LivewireServiceProvider::class])
    ->create();
