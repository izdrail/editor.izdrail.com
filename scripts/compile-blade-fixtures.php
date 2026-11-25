<?php
// Compile actual editor expansions, then syntax-check without rendering them.
require __DIR__.'/fixtures/laravel/vendor/autoload.php';
$app = require __DIR__.'/fixtures/laravel/bootstrap/app.php';
$app->make(Illuminate\Contracts\Console\Kernel::class)->bootstrap();
$blade = $app->make('blade.compiler');
$entries = json_decode(file_get_contents($argv[1]), true, flags: JSON_THROW_ON_ERROR);
if (!is_array($entries) || count($entries) !== 108) {
    throw new RuntimeException('Expected all108 Blade expansions');
}
$failed = 0;
foreach ($entries as $index => $entry) {
    $prefix = $entry['prefix'][0];
    $source = $entry['text'];
    // Fragments require their documented insertion context, not changed contents.
    if ($prefix === 'lv-blade-elseif') $source = '@if(false)'.$source.'@endif';
    if (in_array($prefix, ['lv-blade-slot', 'lv-blade-x-slot'], true)) {
        $source = '<x-name>'.$source.'</x-name>';
    }
    try {
        $compiled = $blade->compileString($source);
        $path = storage_path('framework/views/snippet-'.$index.'.php');
        file_put_contents($path, $compiled);
        $process = proc_open([PHP_BINARY, '-l', $path], [1 => ['pipe', 'w'], 2 => ['pipe', 'w']], $pipes);
        $output = stream_get_contents($pipes[1]).stream_get_contents($pipes[2]);
        fclose($pipes[1]); fclose($pipes[2]);
        if (proc_close($process) !== 0) throw new RuntimeException($output);
        echo $prefix." COMPILE+PHP PASS\n";
    } catch (Throwable $error) {
        $failed++;
        echo $prefix.' FAIL '.$error->getMessage()."\n";
    }
}
echo (count($entries) - $failed).'/'.count($entries)." Blade expansions compiled and passed PHP syntax\n";
exit($failed ? 1 : 0);
