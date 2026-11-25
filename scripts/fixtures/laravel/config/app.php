<?php
return [
    'name' => 'Snippet fixtures', 'env' => 'testing', 'debug' => true,
    'url' => 'http://localhost', 'timezone' => 'UTC', 'locale' => 'en',
    'fallback_locale' => 'en', 'key' => 'base64:'.base64_encode(str_repeat('x', 32)),
    'cipher' => 'AES-256-CBC',
    'providers' => Illuminate\Support\ServiceProvider::defaultProviders()->toArray(),
];
