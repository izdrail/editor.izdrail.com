<?php
namespace App\Http;
Route::middleware(['auth', 'verified'])->get('/items', fn() => view('items'));
$items = Item::where('active', true)->with('owner')->get();
Artisan::call('migrate:status');
class Provider extends ServiceProvider { public function boot(): void { $this->app->singleton(Service::class); } }
